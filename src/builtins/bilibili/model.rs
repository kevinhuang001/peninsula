#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BilibiliEvent {
    pub id: String,
    pub account: String,
    pub kind: EventKind,
    pub title: String,
    pub body: String,
    pub url: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventKind {
    Message,
    Like,
}
