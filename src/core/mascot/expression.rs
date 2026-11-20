use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MascotMotion {
    #[default]
    Idle,
    Blink,
    LookAround,
    Sleepy,
    Happy,
    Excited,
    Thinking,
    Surprised,
    Love,
    Celebrate,
    Listening,
    Focused,
    Charging,
    Typing,
    Verifying,
    Error,
    Success,
}

impl MascotMotion {
    pub fn duration(self) -> Duration {
        use MascotMotion::*;
        Duration::from_millis(match self {
            Blink => 220,
            Error => 1000,
            Success | Happy | Surprised => 850,
            Celebrate | Excited | Love => 1500,
            Sleepy | Thinking => 2300,
            _ => 1800,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Trigger {
    While(Vec<(String, String)>),
    OnEvent { name: String, ttl: Duration },
}

#[derive(Clone, Debug, PartialEq)]
pub struct MascotExpression {
    pub id: String,
    pub name: String,
    pub motion: MascotMotion,
    pub trigger: Trigger,
    pub duration: Duration,
    pub weight: u8,
}

impl MascotExpression {
    pub fn event(id: &str, event: &str, motion: MascotMotion) -> Self {
        Self::new(
            id,
            motion,
            Trigger::OnEvent {
                name: event.into(),
                ttl: Duration::from_secs(15),
            },
        )
    }

    pub fn state(id: &str, key: &str, value: &str, motion: MascotMotion) -> Self {
        Self::new(id, motion, Trigger::While(vec![(key.into(), value.into())]))
    }

    fn new(id: &str, motion: MascotMotion, trigger: Trigger) -> Self {
        Self {
            id: id.into(),
            name: id.into(),
            trigger,
            motion,
            weight: 1,
            duration: motion.duration(),
        }
    }
}
