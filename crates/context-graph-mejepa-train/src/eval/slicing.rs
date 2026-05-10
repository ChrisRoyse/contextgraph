use crate::eval::{Lang, MutationCategory};
use std::collections::HashMap;

pub const GENERIC_ONLY_GAP_THRESHOLD: f32 = 0.10;

pub fn per_mutation_category_accuracy(
    predictions: &[(MutationCategory, bool, bool)],
) -> HashMap<MutationCategory, f32> {
    let mut counts: HashMap<MutationCategory, (u32, u32)> = HashMap::new();
    for (cat, predicted, actual) in predictions {
        let entry = counts.entry(*cat).or_insert((0, 0));
        entry.1 += 1;
        if predicted == actual {
            entry.0 += 1;
        }
    }
    counts
        .into_iter()
        .map(|(k, (correct, total))| (k, correct as f32 / total.max(1) as f32))
        .collect()
}

pub fn per_language_accuracy(predictions: &[(Lang, bool, bool)]) -> HashMap<Lang, f32> {
    let mut counts: HashMap<Lang, (u32, u32)> = HashMap::new();
    for (lang, predicted, actual) in predictions {
        let entry = counts.entry(*lang).or_insert((0, 0));
        entry.1 += 1;
        if predicted == actual {
            entry.0 += 1;
        }
    }
    counts
        .into_iter()
        .map(|(k, (correct, total))| (k, correct as f32 / total.max(1) as f32))
        .collect()
}

pub fn detect_generic_only_warning(
    per_cat: &HashMap<MutationCategory, f32>,
    per_lang: &HashMap<Lang, f32>,
    gap_threshold: f32,
) -> Option<String> {
    let cat = max_gap(per_cat.iter().map(|(k, v)| (format!("{k:?}"), *v)));
    let lang = max_gap(per_lang.iter().map(|(k, v)| (format!("{k:?}"), *v)));
    match (cat, lang) {
        (Some((cl, ch, cg)), Some((ll, lh, lg))) if cg.max(lg) > gap_threshold => {
            if cg >= lg {
                Some(format!("category:{cl}-vs-{ch} gap={cg:.3}"))
            } else {
                Some(format!("language:{ll}-vs-{lh} gap={lg:.3}"))
            }
        }
        (Some((low, high, gap)), None) | (None, Some((low, high, gap))) if gap > gap_threshold => {
            Some(format!("slice:{low}-vs-{high} gap={gap:.3}"))
        }
        _ => None,
    }
}

fn max_gap<I>(iter: I) -> Option<(String, String, f32)>
where
    I: Iterator<Item = (String, f32)>,
{
    let mut min_key = None::<String>;
    let mut max_key = None::<String>;
    let mut min_val = f32::INFINITY;
    let mut max_val = f32::NEG_INFINITY;
    for (key, val) in iter {
        if val < min_val {
            min_val = val;
            min_key = Some(key.clone());
        }
        if val > max_val {
            max_val = val;
            max_key = Some(key);
        }
    }
    Some((min_key?, max_key?, max_val - min_val))
}
