use crate::{PeleError, PeleErrorCode};
use indexmap::IndexMap;
#[derive(Debug, Clone, Default)]
pub struct Node {
    pub name: String,
    pub kind: String,
    pub requirement_id: String,
    pub text: String,
    pub risk: String,
    pub verify_method: String,
    pub doc_ref: String,
    pub css_styles: Vec<String>,
    pub classes: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct Relation {
    pub kind: String,
    pub src: String,
    pub dst: String,
}
#[derive(Debug, Clone, Default)]
pub struct Class {
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct Model {
    pub direction: String,
    pub requirements: IndexMap<String, Node>,
    pub elements: IndexMap<String, Node>,
    pub relations: Vec<Relation>,
    pub classes: IndexMap<String, Class>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub style_copies: usize,
}
impl Default for Model {
    fn default() -> Self {
        Self {
            direction: "TB".into(),
            requirements: IndexMap::new(),
            elements: IndexMap::new(),
            relations: vec![],
            classes: IndexMap::new(),
            title: None,
            acc_title: None,
            acc_descr: None,
            style_copies: 0,
        }
    }
}
fn copy(count: &mut usize, target: &mut Vec<String>, styles: &[String]) -> Result<(), PeleError> {
    *count += styles.len();
    if *count > 1_000_000 {
        return Err(PeleError::new("Requirement diagram applies too many style declarations.", PeleErrorCode::Limit).of("requirement"));
    }
    target.extend_from_slice(styles);
    Ok(())
}
impl Model {
    pub fn style(&mut self, ids: &[String], styles: &[String]) -> Result<(), PeleError> {
        for id in ids {
            let Some(node) = self.requirements.get_mut(id).or_else(|| self.elements.get_mut(id)) else {
                return Ok(());
            };
            for s in styles {
                let parts: Vec<String> = s.split(',').map(str::to_owned).collect();
                copy(&mut self.style_copies, &mut node.css_styles, &parts)?;
            }
        }
        Ok(())
    }
    pub fn set_class(&mut self, ids: &[String], names: &[String]) -> Result<(), PeleError> {
        for id in ids {
            let Some(node) = self.requirements.get_mut(id).or_else(|| self.elements.get_mut(id)) else {
                continue;
            };
            for name in names {
                node.classes.push(name.clone());
                if let Some(class) = self.classes.get(name) {
                    copy(&mut self.style_copies, &mut node.css_styles, &class.styles)?;
                }
            }
        }
        Ok(())
    }
    pub fn define_class(&mut self, ids: &[String], styles: &[String]) -> Result<(), PeleError> {
        for id in ids {
            let class = self.classes.entry(id.clone()).or_default();
            for s in styles {
                if s.contains("color") {
                    class.text_styles.push(s.replacen("fill", "bgFill", 1));
                }
                class.styles.push(s.clone());
            }
            let split: Vec<String> = styles.iter().flat_map(|s| s.split(',').map(str::to_owned)).collect();
            for node in self.requirements.values_mut().chain(self.elements.values_mut()) {
                if node.classes.contains(id) {
                    copy(&mut self.style_copies, &mut node.css_styles, &split)?;
                }
            }
        }
        Ok(())
    }
}
