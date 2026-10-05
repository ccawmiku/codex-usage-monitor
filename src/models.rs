use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BarStyle {
    #[default]
    Segmented,
    Pill,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ColorTheme {
    #[default]
    Emerald,    // OpenAI Green #10A37F
    Coral,      // Claude Warm Coral #D97757
    Cyan,       // Cyber Cyan #00B4D8
    Dynamic,    // Dynamic: Green (>40%) -> Amber (20-40%) -> Red (<20%)
    Monochrome, // Clean White/Black
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TextFormat {
    #[default]
    Compact,    // "65% · 14:59" or "65% · 10/12"
    Verbose,    // "剩余65%  14:59重置"
    Countdown,  // "65% · 3h"
}

#[derive(Clone, Debug, Default)]
pub struct UsageSection {
    pub percentage: f64,
    pub resets_at: Option<SystemTime>,
}

#[derive(Clone, Debug, Default)]
pub struct UsageData {
    pub session: UsageSection,
    pub weekly: UsageSection,
}

#[derive(Clone, Debug, Default)]
pub struct AppUsageData {
    pub claude_code: Option<UsageData>,
    pub codex: Option<UsageData>,
    pub antigravity: Option<UsageData>,
}

