#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GovernmentIdeology {
    #[default]
    Democratic,
    Fascism,
    Communism,
    Neutrality,
}

impl GovernmentIdeology {
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "democratic" => Some(Self::Democratic),
            "fascism" => Some(Self::Fascism),
            "communism" => Some(Self::Communism),
            "neutrality" => Some(Self::Neutrality),
            _ => None,
        }
    }
}
