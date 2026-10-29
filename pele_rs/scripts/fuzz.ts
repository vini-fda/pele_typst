// bun pele_rs/scripts/fuzz.ts <pinned checkout> [cases-per-type=40]
// Build `cargo build -p pele_rs --example oracle` first. Failures are minimized
// and retained as source inputs for fixtures.ts, never accepted as new behavior.
import { existsSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { execFileSync } from 'node:child_process';
import reference from './reference.json';
const upstream=resolve(process.argv[2]);
const revision=existsSync(join(upstream,'.git'))?execFileSync('git',['-C',upstream,'rev-parse','HEAD'],{encoding:'utf8'}).trim():readFileSync(join(upstream,'.pele-reference-revision'),'utf8').trim();
if (revision!==reference.revision || JSON.parse(readFileSync(join(upstream,'package.json'),'utf8')).version!==reference.version) throw new Error('wrong upstream revision/version');
process.env.TZ=reference.timeZone;
const {render}=await import(join(upstream,'src/index.ts'));
const {metricsMeasurer}=await import(join(upstream,'src/text/measurer.ts'));
const binary=resolve(import.meta.dir,'../../target/debug/examples/oracle');
const count=Number(process.argv[3]??40);
const options={now:reference.now,timeZone:'UTC'};
const normalized=(v:any):any=>Array.isArray(v)?v.map(normalized):v&&typeof v==='object'?Object.fromEntries(Object.keys(v).sort().map(k=>[k,normalized(v[k])])):v;
const same=(a:any,b:any)=>JSON.stringify(normalized(a))===JSON.stringify(normalized(b));
function expected(source:string) {
  try {return {result:render(source,{...options,measurer:metricsMeasurer})};}
  catch(e:any) {if(e.name!=='PeleError')throw e;return {error:{message:e.message.toWellFormed(),code:e.code,type:e.type,mermaidType:e.mermaidType,line:e.line,column:e.column,snippet:e.snippet,
    ...(e.message.isWellFormed()?{}:{messageUtf16:Array.from({length:e.message.length},(_,i)=>e.message.charCodeAt(i))})}};}
}
function native(sources:string[]):any[] {
  try {
    const result=execFileSync(binary,[],{input:sources.map(source=>JSON.stringify({source,options})).join('\n')+'\n',encoding:'utf8',maxBuffer:256*1024*1024,stdio:['pipe','pipe','pipe']});
    const rows=result.trim().split('\n').map(s=>JSON.parse(s));
    if(rows.length!==sources.length)throw new Error('native adapter lost cases');
    return rows;
  } catch(error:any) {
    // Isolate process failures so one malformed input does not discard later types.
    if(sources.length>1)return sources.flatMap(source=>native([source]));
    return [{nativeCrash:String(error.stderr??error.message).trim()}];
  }
}
let state=0x70656c65;
function random(n:number){state^=state<<13;state^=state>>>17;state^=state<<5;return (state>>>0)%n;}
const pieces=['\n','%% comment\n','"','[',']','{','}','-->',';','😀','&amp;','#35;','$$x$$','fa:fa-star','\u00a0',' ',':','end','-1','0'];
function mutate(source:string){const chars=Array.from(source);const at=random(chars.length+1);const amount=random(Math.min(20,chars.length-at)+1);chars.splice(at,amount,...Array.from(pieces[random(pieces.length)]));return chars.join('');}
function mismatch(source:string){try{return !same(expected(source),native([source])[0]);}catch{return false;}}
function minimize(source:string){
  let best=source;
  for(let chunk=Math.floor(Array.from(best).length/2);chunk>=1;chunk=Math.floor(chunk/2)){
    for(let at=0;at<Array.from(best).length;){const chars=Array.from(best);const candidate=chars.slice(0,at).concat(chars.slice(at+chunk)).join('');
      if(candidate.length<best.length&&mismatch(candidate))best=candidate;else at+=chunk;
    }
  }
  return best;
}
mkdirSync(join(import.meta.dir,'regressions'),{recursive:true});
let failures=0,total=0;
for(const [kind,corpus] of Object.entries(reference.types)){
  const docs:string[]=JSON.parse(readFileSync(join(upstream,`tests/corpus/${corpus}-docs.json`),'utf8')).filter((s:string)=>s.length<4000);
  if(!docs.length)throw new Error(`no fuzz seeds for ${kind}`);
  const sources=Array.from({length:count},()=>mutate(docs[random(docs.length)]));
  // Consume every family seed even in focused runs, preserving the full-run inputs.
  if(process.env.FUZZ_TYPES && !process.env.FUZZ_TYPES.split(',').includes(kind))continue;
  const actual=native(sources);
  const regressions:any[]=[];
  for(let i=0;i<sources.length;i++){
    total++;
    if(!same(expected(sources[i]),actual[i])){
      failures++;
      const source=minimize(sources[i]);
      if(!regressions.some(c=>c.source===source))regressions.push({source,options});
      console.error(`${kind}: mismatch ${i}, minimized ${sources[i].length} -> ${source.length} characters`);
    }
  }
  if(regressions.length){
    const path=join(import.meta.dir,`regressions/${kind}.json`);
    const retained:any[]=existsSync(path)?JSON.parse(readFileSync(path,'utf8')):[];
    for(const regression of regressions)if(!retained.some(c=>same(c,regression)))retained.push(regression);
    writeFileSync(path,JSON.stringify(retained,null,2)+'\n');
  }
  console.log(`${kind}: ${count} deterministic mutations, ${regressions.length} distinct regressions`);
}
console.log(`${total} cases, ${failures} differences; seed=0x70656c65`);
if(failures)process.exitCode=1;
