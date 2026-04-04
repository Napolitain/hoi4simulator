#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClausewitzValue {
    Integer(i64),
    Decimal(Box<str>),
    Bool(bool),
    String(Box<str>),
    Block(ClausewitzBlock),
}

impl ClausewitzValue {
    pub fn as_block(&self) -> Option<&ClausewitzBlock> {
        match self {
            Self::Block(block) => Some(block),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            Self::Decimal(value) => value.parse::<f64>().ok().map(|number| number as i64),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        self.as_i64().and_then(|value| u64::try_from(value).ok())
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Integer(value) => Some(*value as f64),
            Self::Decimal(value) => value.parse::<f64>().ok(),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value.as_ref()),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClausewitzAssignment {
    pub key: Box<str>,
    pub operator: ClausewitzOperator,
    pub value: ClausewitzValue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClausewitzOperator {
    Assign,
    GreaterThan,
    GreaterOrEqual,
    LessThan,
    LessOrEqual,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClausewitzItem {
    Assignment(ClausewitzAssignment),
    Value(ClausewitzValue),
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ClausewitzBlock {
    pub items: Vec<ClausewitzItem>,
}

impl ClausewitzBlock {
    pub fn first_assignment(&self, key: &str) -> Option<&ClausewitzValue> {
        self.items.iter().find_map(|item| match item {
            ClausewitzItem::Assignment(assignment)
                if assignment.key.as_ref() == key
                    && assignment.operator == ClausewitzOperator::Assign =>
            {
                Some(&assignment.value)
            }
            _ => None,
        })
    }

    pub fn assignments<'a>(
        &'a self,
        key: &'a str,
    ) -> impl Iterator<Item = &'a ClausewitzValue> + 'a {
        self.items.iter().filter_map(move |item| match item {
            ClausewitzItem::Assignment(assignment)
                if assignment.key.as_ref() == key
                    && assignment.operator == ClausewitzOperator::Assign =>
            {
                Some(&assignment.value)
            }
            _ => None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    LeftBrace,
    RightBrace,
    Equals,
    GreaterThan,
    GreaterOrEqual,
    LessThan,
    LessOrEqual,
    Atom(Box<str>),
    Quoted(Box<str>),
}

pub fn parse_clausewitz(content: &str) -> Result<ClausewitzBlock, String> {
    let tokens = tokenize(content)?;
    let mut parser = Parser { tokens, index: 0 };
    parser.parse_root()
}

fn tokenize(content: &str) -> Result<Vec<Token>, String> {
    let chars = content.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0_usize;

    while index < chars.len() {
        match chars[index] {
            '{' => {
                tokens.push(Token::LeftBrace);
                index += 1;
            }
            '}' => {
                tokens.push(Token::RightBrace);
                index += 1;
            }
            '=' => {
                tokens.push(Token::Equals);
                index += 1;
            }
            '>' => {
                if index + 1 < chars.len() && chars[index + 1] == '=' {
                    tokens.push(Token::GreaterOrEqual);
                    index += 2;
                } else {
                    tokens.push(Token::GreaterThan);
                    index += 1;
                }
            }
            '<' => {
                if index + 1 < chars.len() && chars[index + 1] == '=' {
                    tokens.push(Token::LessOrEqual);
                    index += 2;
                } else {
                    tokens.push(Token::LessThan);
                    index += 1;
                }
            }
            '"' => {
                index += 1;
                let mut value = String::new();
                while index < chars.len() {
                    let ch = chars[index];
                    if ch == '"' {
                        index += 1;
                        break;
                    }
                    if ch == '\\' && index + 1 < chars.len() {
                        index += 1;
                        value.push(chars[index]);
                        index += 1;
                        continue;
                    }
                    value.push(ch);
                    index += 1;
                }
                tokens.push(Token::Quoted(value.into_boxed_str()));
            }
            '#' => {
                while index < chars.len() && chars[index] != '\n' {
                    index += 1;
                }
            }
            ch if ch.is_whitespace() || ch == '\u{feff}' => {
                index += 1;
            }
            _ => {
                let start = index;
                while index < chars.len()
                    && !chars[index].is_whitespace()
                    && !matches!(chars[index], '{' | '}' | '=' | '>' | '<' | '#')
                {
                    index += 1;
                }
                let atom = chars[start..index].iter().collect::<String>();
                tokens.push(Token::Atom(atom.into_boxed_str()));
            }
        }
    }

    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    fn parse_root(&mut self) -> Result<ClausewitzBlock, String> {
        let mut items = Vec::new();

        while self.index < self.tokens.len() {
            if matches!(self.peek(), Some(Token::RightBrace)) {
                return Err("unexpected closing brace".to_string());
            }
            items.push(self.parse_item()?);
        }

        Ok(ClausewitzBlock { items })
    }

    fn parse_block(&mut self) -> Result<ClausewitzBlock, String> {
        self.expect_left_brace()?;
        let mut items = Vec::new();

        while !matches!(self.peek(), Some(Token::RightBrace) | None) {
            items.push(self.parse_item()?);
        }

        if matches!(self.peek(), Some(Token::RightBrace)) {
            self.expect_right_brace()?;
        }
        Ok(ClausewitzBlock { items })
    }

    fn parse_item(&mut self) -> Result<ClausewitzItem, String> {
        if self.looks_like_assignment() {
            let key = self.parse_key()?;
            let operator = self.parse_operator()?;
            let value = self.parse_value()?;
            return Ok(ClausewitzItem::Assignment(ClausewitzAssignment {
                key,
                operator,
                value,
            }));
        }

        Ok(ClausewitzItem::Value(self.parse_value()?))
    }

    fn parse_key(&mut self) -> Result<Box<str>, String> {
        match self.next() {
            Some(Token::Atom(value)) | Some(Token::Quoted(value)) => Ok(value),
            _ => Err("expected assignment key".to_string()),
        }
    }

    fn parse_value(&mut self) -> Result<ClausewitzValue, String> {
        match self.peek() {
            Some(Token::LeftBrace) => Ok(ClausewitzValue::Block(self.parse_block()?)),
            Some(Token::Atom(_)) | Some(Token::Quoted(_)) => self.parse_scalar_value(),
            Some(
                Token::GreaterThan | Token::GreaterOrEqual | Token::LessThan | Token::LessOrEqual,
            ) => Err("unexpected comparison operator while reading value".to_string()),
            Some(Token::RightBrace) => {
                Err("unexpected closing brace while reading value".to_string())
            }
            Some(Token::Equals) | None => Err("expected value".to_string()),
        }
    }

    fn parse_scalar_value(&mut self) -> Result<ClausewitzValue, String> {
        match self.next() {
            Some(Token::Quoted(value)) => Ok(ClausewitzValue::String(value)),
            Some(Token::Atom(value)) => Ok(atom_to_value(&value)),
            _ => Err("expected scalar value".to_string()),
        }
    }

    fn looks_like_assignment(&self) -> bool {
        matches!(self.peek(), Some(Token::Atom(_) | Token::Quoted(_)))
            && matches!(
                self.peek_n(1),
                Some(
                    Token::Equals
                        | Token::GreaterThan
                        | Token::GreaterOrEqual
                        | Token::LessThan
                        | Token::LessOrEqual
                )
            )
    }

    fn expect_left_brace(&mut self) -> Result<(), String> {
        match self.next() {
            Some(Token::LeftBrace) => Ok(()),
            _ => Err("expected '{'".to_string()),
        }
    }

    fn expect_right_brace(&mut self) -> Result<(), String> {
        match self.next() {
            Some(Token::RightBrace) => Ok(()),
            _ => Err("expected '}'".to_string()),
        }
    }

    fn parse_operator(&mut self) -> Result<ClausewitzOperator, String> {
        match self.next() {
            Some(Token::Equals) => Ok(ClausewitzOperator::Assign),
            Some(Token::GreaterThan) => Ok(ClausewitzOperator::GreaterThan),
            Some(Token::GreaterOrEqual) => Ok(ClausewitzOperator::GreaterOrEqual),
            Some(Token::LessThan) => Ok(ClausewitzOperator::LessThan),
            Some(Token::LessOrEqual) => Ok(ClausewitzOperator::LessOrEqual),
            _ => Err("expected comparison operator".to_string()),
        }
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.index).cloned();
        if token.is_some() {
            self.index += 1;
        }
        token
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn peek_n(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.index + offset)
    }
}

fn atom_to_value(atom: &str) -> ClausewitzValue {
    if atom.eq_ignore_ascii_case("yes") || atom.eq_ignore_ascii_case("true") {
        return ClausewitzValue::Bool(true);
    }
    if atom.eq_ignore_ascii_case("no") || atom.eq_ignore_ascii_case("false") {
        return ClausewitzValue::Bool(false);
    }
    if let Ok(value) = atom.parse::<i64>() {
        return ClausewitzValue::Integer(value);
    }
    if atom.matches('.').count() == 1
        && atom
            .chars()
            .all(|ch| ch == '.' || ch == '-' || ch.is_ascii_digit())
        && atom.parse::<f64>().is_ok()
    {
        return ClausewitzValue::Decimal(atom.to_string().into_boxed_str());
    }

    ClausewitzValue::String(atom.to_string().into_boxed_str())
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{ClausewitzItem, ClausewitzOperator, ClausewitzValue, parse_clausewitz};

    #[test]
    fn parser_handles_nested_blocks_and_scalar_lists() {
        let parsed = parse_clausewitz(
            r#"
            add_ideas = { civilian_economy export_focus }
            state = {
                id = 1
                history = {
                    owner = FRA
                    buildings = { industrial_complex = 4 arms_factory = 2 }
                }
            }
            "#,
        )
        .unwrap();

        let add_ideas = parsed
            .first_assignment("add_ideas")
            .unwrap()
            .as_block()
            .unwrap();
        assert!(matches!(
            add_ideas.items[0],
            ClausewitzItem::Value(ClausewitzValue::String(_))
        ));

        let state = parsed
            .first_assignment("state")
            .unwrap()
            .as_block()
            .unwrap();
        let history = state
            .first_assignment("history")
            .unwrap()
            .as_block()
            .unwrap();
        let buildings = history
            .first_assignment("buildings")
            .unwrap()
            .as_block()
            .unwrap();

        assert_eq!(state.first_assignment("id").unwrap().as_i64(), Some(1));
        assert_eq!(
            history.first_assignment("owner").unwrap().as_str(),
            Some("FRA")
        );
        assert_eq!(
            buildings
                .first_assignment("industrial_complex")
                .unwrap()
                .as_i64(),
            Some(4)
        );
    }

    #[test]
    fn parser_ignores_comments() {
        let parsed = parse_clausewitz(
            r#"
            state = {
                id = 12 # comment
                manpower = 8000000
            }
            "#,
        )
        .unwrap();

        let state = parsed
            .first_assignment("state")
            .unwrap()
            .as_block()
            .unwrap();
        assert_eq!(state.first_assignment("id").unwrap().as_i64(), Some(12));
        assert_eq!(
            state.first_assignment("manpower").unwrap().as_i64(),
            Some(8_000_000)
        );
    }

    proptest! {
        #[test]
        fn parser_preserves_integer_assignments_with_comment_suffix(value in 0i64..1_000_000) {
            let content = format!(
                "state = {{ id = {value} # trailing comment\n manpower = {value} }}"
            );
            let parsed = parse_clausewitz(&content).unwrap();
            let state = parsed.first_assignment("state").unwrap().as_block().unwrap();

            prop_assert_eq!(state.first_assignment("id").unwrap().as_i64(), Some(value));
            prop_assert_eq!(state.first_assignment("manpower").unwrap().as_i64(), Some(value));
        }
    }

    #[test]
    fn parser_accepts_eof_terminated_final_block() {
        let parsed = parse_clausewitz("instant_effect = { amount = 2").unwrap();

        let block = parsed
            .first_assignment("instant_effect")
            .unwrap()
            .as_block()
            .unwrap();
        assert_eq!(block.first_assignment("amount").unwrap().as_i64(), Some(2));
    }

    #[test]
    fn parser_preserves_comparison_operators() {
        let parsed = parse_clausewitz(
            "available = { has_war_support > 0.12 amount_research_slots < 5 free_building_slots >= 2 infrastructure <= 3 }",
        )
        .unwrap();
        let available = parsed
            .first_assignment("available")
            .unwrap()
            .as_block()
            .unwrap();

        let operators = available
            .items
            .iter()
            .filter_map(|item| match item {
                ClausewitzItem::Assignment(assignment) => {
                    Some((assignment.key.as_ref(), assignment.operator))
                }
                ClausewitzItem::Value(_) => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            operators,
            vec![
                ("has_war_support", ClausewitzOperator::GreaterThan),
                ("amount_research_slots", ClausewitzOperator::LessThan),
                ("free_building_slots", ClausewitzOperator::GreaterOrEqual),
                ("infrastructure", ClausewitzOperator::LessOrEqual),
            ]
        );
    }

    #[test]
    fn parser_ignores_utf8_bom_prefix() {
        let parsed = parse_clausewitz("\u{feff}focus_tree = { id = french_focus }").unwrap();
        let focus_tree = parsed
            .first_assignment("focus_tree")
            .unwrap()
            .as_block()
            .unwrap();

        assert_eq!(
            focus_tree.first_assignment("id").unwrap().as_str(),
            Some("french_focus")
        );
    }
}
