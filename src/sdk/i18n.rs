use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct Translator {
    locale: String,
    messages: HashMap<(String, String), String>,
}

impl Translator {
    pub fn new(locale: impl Into<String>) -> Self {
        Self {
            locale: locale.into(),
            messages: HashMap::new(),
        }
    }
    pub fn insert(
        &mut self,
        locale: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) {
        self.messages
            .insert((locale.into(), key.into()), value.into());
    }
    pub fn text<'a>(&'a self, key: &'a str) -> &'a str {
        self.messages
            .get(&(self.locale.clone(), key.to_owned()))
            .or_else(|| self.messages.get(&("en".into(), key.to_owned())))
            .map_or(key, String::as_str)
    }
}
