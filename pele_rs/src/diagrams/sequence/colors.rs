const KEYWORDS: &str = "aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet brown burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue darkcyan darkgoldenrod darkgray darkgreen darkgrey darkkhaki darkmagenta darkolivegreen darkorange darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray darkslategrey darkturquoise darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia gainsboro ghostwhite gold goldenrod gray green greenyellow grey honeydew hotpink indianred indigo ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon lightseagreen lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta maroon mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue mediumspringgreen mediumturquoise mediumvioletred midnightblue mintcream mistyrose moccasin navajowhite navy oldlace olive olivedrab orange orangered orchid palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue purple rebeccapurple red rosybrown royalblue saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue slategray slategrey snow springgreen steelblue tan teal thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen transparent currentcolor inherit initial unset revert accentcolor accentcolortext activetext buttonborder buttonface buttontext canvas canvastext field fieldtext graytext highlight highlighttext linktext mark marktext selecteditem selecteditemtext visitedtext activeborder activecaption appworkspace background buttonhighlight buttonshadow captiontext inactiveborder inactivecaption inactivecaptiontext infobackground infotext menu menutext scrollbar threeddarkshadow threedface threedhighlight threedlightshadow threedshadow window windowframe windowtext";
use regex::Regex;
use std::sync::OnceLock;
fn kind(term: &str) -> u8 {
    if term.eq_ignore_ascii_case("none") {
        return 4;
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    let Some(m) = RE
        .get_or_init(|| Regex::new(r"(?i)^[+-]?(?:\d+(?:\.\d+)?|\.\d+)(?:e[+-]?\d+)?(%|deg|grad|rad|turn)?$").unwrap())
        .captures(term)
    else {
        return 0;
    };
    match m.get(1).map(|m| m.as_str()) {
        None => 1,
        Some("%") => 2,
        _ => 3,
    }
}
pub fn is_css_color(color: &str) -> bool {
    let Some(open) = color.find('(') else {
        return KEYWORDS.split(' ').any(|s| s.eq_ignore_ascii_case(color));
    };
    let hsl = color.starts_with("hsl");
    if !matches!(&color[..open], "rgb" | "rgba" | "hsl" | "hsla") || color.len() > 120 || color.find(')') != Some(color.len() - 1) {
        return false;
    }
    let inner = color[open + 1..color.len() - 1].trim();
    let legacy = inner.contains(',');
    let mut alpha = None;
    let parts: Vec<&str> = if legacy {
        let mut parts: Vec<_> = inner.split(',').map(str::trim).collect();
        if parts.len() == 4 {
            alpha = parts.pop();
        }
        parts
    } else {
        let slash: Vec<_> = inner.split('/').collect();
        if slash.len() > 2 {
            return false;
        }
        if slash.len() == 2 {
            alpha = Some(slash[1].trim());
        }
        slash[0].split_whitespace().collect()
    };
    if parts.len() != 3
        || alpha.is_some_and(|a| {
            let k = kind(a);
            k != 1 && k != 2 && (legacy || k != 4)
        })
    {
        return false;
    }
    let k: Vec<_> = parts.iter().map(|s| kind(s)).collect();
    if legacy {
        if hsl {
            matches!(k[0], 1 | 3) && k[1] == 2 && k[2] == 2
        } else {
            matches!(k[0], 1 | 2) && k[1] == k[0] && k[2] == k[0]
        }
    } else {
        k.iter()
            .enumerate()
            .all(|(i, &k)| k == 1 || k == 4 || if hsl && i == 0 { k == 3 } else { k == 2 })
    }
}
