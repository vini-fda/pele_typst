use crate::{IconResolver, MathMarkup, MathResolver, RenderOptions, TextMeasurer};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Resolver state belongs to one render, including all responsive layout attempts.
pub struct RenderContext<'a> {
    base: &'a dyn TextMeasurer,
    icons: Option<IconResolver>,
    raw_icons: Option<IconResolver>,
    math: Option<MathResolver>,
}

impl MathMarkup {
    pub fn width(&self) -> f64 {
        match self {
            Self::Svg { width, .. } | Self::Html { width, .. } => *width,
        }
    }
    pub fn height(&self) -> f64 {
        match self {
            Self::Svg { height, .. } | Self::Html { height, .. } => *height,
        }
    }
}

impl<'a> RenderContext<'a> {
    pub fn new(base: &'a dyn TextMeasurer, options: &mut RenderOptions) -> Self {
        let mut cached_icons = None;
        if let Some(resolver) = options.icons.take() {
            let raw = IconResolver::new(move |name| resolver.resolve(&crate::text::entities::decode_entities(name)));
            options.icons = Some(raw.clone());
            let found = Arc::new(Mutex::new(HashMap::<String, Option<String>>::new()));
            cached_icons = Some(IconResolver::new(move |name| {
                if let Some(value) = found.lock().unwrap().get(name).cloned() {
                    return value;
                }
                let value = raw.resolve(name);
                found.lock().unwrap().insert(name.into(), value.clone());
                value
            }));
        }
        if let Some(resolver) = options.math.take() {
            let found = Arc::new(Mutex::new(HashMap::<(String, u64), Option<MathMarkup>>::new()));
            options.math = Some(MathResolver::new(move |tex, size| {
                let key = (tex.to_string(), size.to_bits());
                if let Some(value) = found.lock().unwrap().get(&key).cloned() {
                    return value;
                }
                let value = resolver
                    .resolve(tex, size)
                    .filter(|m| m.width().is_finite() && m.width() > 0.0 && m.height().is_finite() && m.height() > 0.0);
                found.lock().unwrap().insert(key, value.clone());
                value
            }));
        }
        Self {
            base,
            icons: cached_icons,
            raw_icons: options.icons.clone(),
            math: options.math.clone(),
        }
    }
}

impl TextMeasurer for RenderContext<'_> {
    fn width(&self, text: &str, size: f64, style: u32) -> f64 {
        self.base.width(text, size, style)
    }
    fn icon_markup(&self, name: &str) -> String {
        self.icons.as_ref().and_then(|r| r.resolve(name)).unwrap_or_default()
    }
    fn icon_resolver(&self) -> Option<IconResolver> {
        self.raw_icons.clone()
    }
    fn math_active(&self) -> bool {
        self.math.is_some()
    }
    fn typeset(&self, tex: &str, size: f64) -> Option<MathMarkup> {
        self.math.as_ref().and_then(|r| r.resolve(tex, size))
    }
}

pub struct WithoutMath<'a>(pub &'a dyn TextMeasurer);
impl TextMeasurer for WithoutMath<'_> {
    fn width(&self, text: &str, size: f64, style: u32) -> f64 {
        self.0.width(text, size, style)
    }
    fn icon_markup(&self, name: &str) -> String {
        self.0.icon_markup(name)
    }
    fn icon_resolver(&self) -> Option<IconResolver> {
        self.0.icon_resolver()
    }
}
