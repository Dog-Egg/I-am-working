use serde_json::{Map, Value};

#[allow(dead_code)]
pub(crate) fn insert_value_at_path(
    json: &str,
    path: &[&str],
    value: Value,
) -> Result<String, String> {
    if path.is_empty() {
        return Err("insert path cannot be empty".to_string());
    }

    let mut root: Value = serde_json::from_str(json).map_err(|err| err.to_string())?;
    let mut current = root
        .as_object_mut()
        .ok_or_else(|| "input JSON root must be an object".to_string())?;

    for key in &path[..path.len() - 1] {
        current = ensure_child_object(current, key)?;
    }

    let leaf_key = path[path.len() - 1];
    match current.get_mut(leaf_key) {
        Some(Value::Array(items)) => items.push(value),
        Some(_) => return Err(format!("target path '{leaf_key}' must be an array")),
        None => {
            current.insert(leaf_key.to_string(), Value::Array(vec![value]));
        }
    }

    serde_json::to_string_pretty(&root).map_err(|err| err.to_string())
}

fn ensure_child_object<'a>(
    parent: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Map<String, Value>, String> {
    let child = parent
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));

    child
        .as_object_mut()
        .ok_or_else(|| format!("path segment '{key}' must be an object"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn inserts_value_into_missing_path() {
        let hook = json!({
            "hooks": [
                {
                    "type": "command",
                    "command": "<shell command>",
                    "timeout": 30
                }
            ]
        });

        let output = insert_value_at_path("{}", &["hooks", "my-event"], hook.clone()).unwrap();
        let output: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(
            output,
            json!({
                "hooks": {
                    "my-event": [hook]
                }
            })
        );
    }

    #[test]
    fn preserves_existing_hooks_and_inserts_new_event() {
        let input = r#"
        {
          "version": 1,
          "hooks": {
            "<EventName>": [
              {
                "matcher": "<ToolPattern>",
                "loop_limit": 5,
                "hooks": [
                  {
                    "type": "command",
                    "command": "<shell command>",
                    "timeout": 30
                  }
                ]
              }
            ]
          }
        }
        "#;
        let hook = json!({
            "hooks": [
                {
                    "type": "command",
                    "command": "<shell command>",
                    "timeout": 30
                }
            ]
        });

        let output = insert_value_at_path(input, &["hooks", "my-event"], hook.clone()).unwrap();
        let output: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(output["version"], json!(1));
        assert_eq!(
            output["hooks"]["<EventName>"][0]["matcher"],
            json!("<ToolPattern>")
        );
        assert_eq!(output["hooks"]["my-event"], json!([hook]));
    }

    #[test]
    fn appends_value_to_existing_target_array() {
        let input = r#"{"hooks":{"my-event":[{"hooks":[]}]}}"#;
        let hook = json!({
            "hooks": [
                {
                    "type": "command",
                    "command": "<shell command>",
                    "timeout": 30
                }
            ]
        });

        let output = insert_value_at_path(input, &["hooks", "my-event"], hook.clone()).unwrap();
        let output: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(output["hooks"]["my-event"], json!([{"hooks": []}, hook]));
    }
}
