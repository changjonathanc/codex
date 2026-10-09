//! Validated RGB color for the final-answer left rail.

use schemars::JsonSchema;
use schemars::r#gen::SchemaGenerator;
use schemars::schema::InstanceType;
use schemars::schema::Schema;
use schemars::schema::SchemaObject;
use schemars::schema::StringValidation;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FinalMessageColor([u8; 3]);

impl FinalMessageColor {
    pub fn rgb(self) -> [u8; 3] {
        self.0
    }
}

impl TryFrom<String> for FinalMessageColor {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        const ERROR: &str = "final_message_color must be a six-digit hex color such as #89b4fa";
        if value.len() != 7
            || !value.starts_with('#')
            || !value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
        {
            return Err(ERROR);
        }
        let mut rgb = [0; 3];
        for (index, channel) in rgb.iter_mut().enumerate() {
            let start = 1 + index * 2;
            *channel = u8::from_str_radix(&value[start..start + 2], 16).map_err(|_| ERROR)?;
        }
        Ok(Self(rgb))
    }
}

impl From<FinalMessageColor> for String {
    fn from(color: FinalMessageColor) -> Self {
        let [red, green, blue] = color.0;
        format!("#{red:02x}{green:02x}{blue:02x}")
    }
}

impl JsonSchema for FinalMessageColor {
    fn schema_name() -> String {
        "FinalMessageColor".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            string: Some(Box::new(StringValidation {
                pattern: Some("^#[0-9a-fA-F]{6}$".into()),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

#[cfg(test)]
#[path = "final_message_color_tests.rs"]
mod tests;
