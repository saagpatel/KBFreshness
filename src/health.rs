use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Green,
    Yellow,
    Red,
}

/// Compute health status based on article age and broken link count.
///
/// Rules:
/// - GREEN: effective_age <= threshold AND broken_count == 0 AND NOT manually_flagged
/// - YELLOW: (effective_age > threshold AND effective_age <= threshold * 2)
///           OR (broken_count >= 1 AND broken_count <= 2)
/// - RED: effective_age > threshold * 2 OR broken_count > 2 OR manually_flagged
pub fn compute_health(
    effective_age_days: i64,
    broken_link_count: i64,
    threshold_days: i64,
    manually_flagged: bool,
) -> HealthStatus {
    // Manual flag immediately makes it red
    if manually_flagged {
        return HealthStatus::Red;
    }

    // Red conditions
    if effective_age_days > threshold_days * 2 || broken_link_count > 2 {
        return HealthStatus::Red;
    }

    // Yellow conditions
    if (effective_age_days > threshold_days && effective_age_days <= threshold_days * 2)
        || (broken_link_count >= 1 && broken_link_count <= 2)
    {
        return HealthStatus::Yellow;
    }

    // Green: fresh and no broken links
    HealthStatus::Green
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_green_fresh_no_broken_links() {
        assert_eq!(compute_health(30, 0, 90, false), HealthStatus::Green);
        assert_eq!(compute_health(89, 0, 90, false), HealthStatus::Green);
        assert_eq!(compute_health(0, 0, 90, false), HealthStatus::Green);
    }

    #[test]
    fn test_yellow_age_threshold() {
        assert_eq!(compute_health(91, 0, 90, false), HealthStatus::Yellow);
        assert_eq!(compute_health(120, 0, 90, false), HealthStatus::Yellow);
        assert_eq!(compute_health(180, 0, 90, false), HealthStatus::Yellow);
    }

    #[test]
    fn test_yellow_broken_links() {
        assert_eq!(compute_health(30, 1, 90, false), HealthStatus::Yellow);
        assert_eq!(compute_health(30, 2, 90, false), HealthStatus::Yellow);
    }

    #[test]
    fn test_red_old_age() {
        assert_eq!(compute_health(181, 0, 90, false), HealthStatus::Red);
        assert_eq!(compute_health(500, 0, 90, false), HealthStatus::Red);
    }

    #[test]
    fn test_red_many_broken_links() {
        assert_eq!(compute_health(30, 3, 90, false), HealthStatus::Red);
        assert_eq!(compute_health(30, 10, 90, false), HealthStatus::Red);
    }

    #[test]
    fn test_red_manually_flagged() {
        assert_eq!(compute_health(30, 0, 90, true), HealthStatus::Red);
        assert_eq!(compute_health(0, 0, 90, true), HealthStatus::Red);
    }

    #[test]
    fn test_custom_threshold() {
        // threshold = 180 days
        assert_eq!(compute_health(100, 0, 180, false), HealthStatus::Green);
        assert_eq!(compute_health(200, 0, 180, false), HealthStatus::Yellow);
        assert_eq!(compute_health(361, 0, 180, false), HealthStatus::Red);
    }

    #[test]
    fn test_worst_case_wins() {
        // Old age (red) + broken links (yellow) = red
        assert_eq!(compute_health(200, 1, 90, false), HealthStatus::Red);
    }
}
