use egui::Ui;
use game_tool_core::ModifiableField;
use std::collections::BTreeMap;

const SPLIT_THRESHOLD: usize = 200;
const SPLIT_SIZE: usize = 100;

pub const CATEGORY_LABELS: &[(&str, &str)] = &[
    ("gold", "金币"),
    ("switch", "开关"),
    ("variable", "变量"),
    ("actor", "角色"),
    ("item", "物品"),
    ("weapon", "武器"),
    ("armor", "防具"),
    ("self_switch", "自开关"),
    ("meta", "元数据"),
    ("gvas", "GVAS"),
    ("general", "通用"),
    ("store", "Store"),
];

pub fn category_display_name(raw: &str) -> &str {
    for (key, label) in CATEGORY_LABELS {
        if *key == raw {
            return label;
        }
    }
    raw
}

pub fn render(ui: &mut Ui, fields: &[ModifiableField], selected: &mut Option<String>) {
    let mut cats: BTreeMap<String, usize> = BTreeMap::new();
    for f in fields {
        *cats.entry(f.category.clone()).or_default() += 1;
    }

    ui.strong("分类");
    ui.add_space(4.0);

    if ui
        .selectable_label(selected.is_none(), format!("全部 ({})", fields.len()))
        .clicked()
    {
        *selected = None;
    }

    for (key, label) in CATEGORY_LABELS {
        if let Some(&count) = cats.get(*key) {
            if count > SPLIT_THRESHOLD {
                // Split into sub-ranges based on max item_id
                let text = format!("{} ({})", label, count);
                ui.label(text);
                let max_id = fields
                    .iter()
                    .filter(|f| f.category == *key)
                    .map(|f| f.item_id as usize)
                    .max()
                    .unwrap_or(count.saturating_sub(1));
                let groups = (max_id / SPLIT_SIZE) + 1;
                for g in 0..groups {
                    let start = g * SPLIT_SIZE;
                    let end = ((g + 1) * SPLIT_SIZE - 1).min(max_id);
                    let sub_text = format!("  {}-{}", start, end);
                    let range_key = format!("{}:{}-{}", key, start, end);
                    let is_sel = selected.as_deref() == Some(&range_key);
                    if ui.selectable_label(is_sel, sub_text).clicked() {
                        *selected = Some(range_key);
                    }
                }
            } else {
                let text = format!("{} ({})", label, count);
                let is_sel = selected.as_deref() == Some(key);
                if ui.selectable_label(is_sel, text).clicked() {
                    *selected = Some(key.to_string());
                }
            }
        }
    }

    for (cat, count) in &cats {
        if !CATEGORY_LABELS.iter().any(|(k, _)| k == cat) {
            let text = format!("{} ({})", cat, count);
            let is_sel = selected.as_deref() == Some(cat.as_str());
            if ui.selectable_label(is_sel, text).clicked() {
                *selected = Some(cat.clone());
            }
        }
    }
}

pub fn render_horizontal(ui: &mut Ui, fields: &[ModifiableField], selected: &mut Option<String>) {
    let mut cats: BTreeMap<String, usize> = BTreeMap::new();
    for f in fields {
        *cats.entry(f.category.clone()).or_default() += 1;
    }

    ui.horizontal_wrapped(|ui| {
        if ui
            .selectable_label(selected.is_none(), format!("全部 ({})", fields.len()))
            .clicked()
        {
            *selected = None;
        }

        for (key, label) in CATEGORY_LABELS {
            if let Some(&count) = cats.get(*key) {
                let text = format!("{} ({})", label, count);
                let is_sel = selected.as_deref() == Some(key);
                if ui.selectable_label(is_sel, text).clicked() {
                    *selected = Some(key.to_string());
                }
            }
        }

        for (cat, count) in &cats {
            if !CATEGORY_LABELS.iter().any(|(k, _)| k == cat) {
                let text = format!("{} ({})", cat, count);
                let is_sel = selected.as_deref() == Some(cat.as_str());
                if ui.selectable_label(is_sel, text).clicked() {
                    *selected = Some(cat.clone());
                }
            }
        }
    });
}

/// Parse a range filter like "switch:0-99" into (category, start, end)
pub fn parse_range(selected: &Option<String>) -> (Option<String>, Option<(usize, usize)>) {
    if let Some(ref sel) = selected {
        if let Some(colon) = sel.find(':') {
            let cat = sel[..colon].to_string();
            let range_str = &sel[colon + 1..];
            if let Some(dash) = range_str.find('-') {
                let start: usize = range_str[..dash].parse().unwrap_or(0);
                let end: usize = range_str[dash + 1..].parse().unwrap_or(0);
                if start <= end {
                    return (Some(cat), Some((start, end)));
                }
            }
            return (Some(cat), None);
        }
        (Some(sel.clone()), None)
    } else {
        (None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_labels_has_12_entries() {
        assert_eq!(CATEGORY_LABELS.len(), 12);
    }

    #[test]
    fn test_category_labels_no_duplicate_keys() {
        let mut keys = std::collections::HashSet::new();
        for (key, _) in CATEGORY_LABELS {
            assert!(keys.insert(*key), "Duplicate key: {}", key);
        }
    }

    #[test]
    fn test_category_labels_no_empty_labels() {
        for (_, label) in CATEGORY_LABELS {
            assert!(!label.is_empty(), "Empty label for a category");
        }
    }

    #[test]
    fn test_category_display_name_known_keys() {
        assert_eq!(category_display_name("gold"), "金币");
        assert_eq!(category_display_name("switch"), "开关");
        assert_eq!(category_display_name("variable"), "变量");
        assert_eq!(category_display_name("actor"), "角色");
        assert_eq!(category_display_name("item"), "物品");
        assert_eq!(category_display_name("weapon"), "武器");
        assert_eq!(category_display_name("armor"), "防具");
        assert_eq!(category_display_name("self_switch"), "自开关");
        assert_eq!(category_display_name("meta"), "元数据");
        assert_eq!(category_display_name("gvas"), "GVAS");
        assert_eq!(category_display_name("general"), "通用");
        assert_eq!(category_display_name("store"), "Store");
    }

    #[test]
    fn test_category_display_name_unknown_passthrough() {
        assert_eq!(category_display_name("unknown_cat"), "unknown_cat");
        assert_eq!(category_display_name("custom_type"), "custom_type");
    }

    #[test]
    fn test_category_display_name_empty_string() {
        assert_eq!(category_display_name(""), "");
    }

    #[test]
    fn test_parse_range_none() {
        let (cat, range) = parse_range(&None);
        assert!(cat.is_none());
        assert!(range.is_none());
    }

    #[test]
    fn test_parse_range_no_colon() {
        let (cat, range) = parse_range(&Some("switch".into()));
        assert_eq!(cat, Some("switch".into()));
        assert!(range.is_none());
    }

    #[test]
    fn test_parse_range_valid() {
        let (cat, range) = parse_range(&Some("switch:0-99".into()));
        assert_eq!(cat, Some("switch".into()));
        assert_eq!(range, Some((0, 99)));
    }

    #[test]
    fn test_parse_range_invalid_numbers() {
        let (cat, range) = parse_range(&Some("switch:abc-def".into()));
        assert_eq!(cat, Some("switch".into()));
        assert_eq!(range, Some((0, 0)));
    }

    #[test]
    fn test_parse_range_colon_no_dash() {
        let (cat, range) = parse_range(&Some("switch:50".into()));
        assert_eq!(cat, Some("switch".into()));
        assert!(range.is_none());
    }

    #[test]
    fn test_parse_range_empty_string() {
        let (cat, range) = parse_range(&Some("".into()));
        assert_eq!(cat, Some("".into()));
        assert!(range.is_none());
    }

    #[test]
    fn test_split_threshold_and_size() {
        assert_eq!(SPLIT_THRESHOLD, 200);
        assert_eq!(SPLIT_SIZE, 100);
    }
}
