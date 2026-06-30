use std::fmt::Write as _;

#[derive(Clone)]
pub struct Report {
    pub command: &'static str,
    pub findings: Vec<Finding>,
}

impl Report {
    pub fn new(command: &'static str) -> Self {
        Self {
            command,
            findings: Vec::new(),
        }
    }

    pub fn extend(&mut self, other: Report) {
        self.findings.extend(other.findings);
    }

    pub fn ok(&self) -> bool {
        !self
            .findings
            .iter()
            .any(|finding| finding.severity == Severity::Fail)
    }

    pub fn has_failure(&self, id: &str) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.id == id && finding.severity == Severity::Fail)
    }

    pub fn skip_if_failed(
        &mut self,
        prerequisite_id: &str,
        skipped_id: &'static str,
        provider: &'static str,
        summary: &str,
    ) -> bool {
        if self.has_failure(prerequisite_id) {
            self.findings.push(Finding::skipped(
                skipped_id,
                provider,
                "blocked",
                summary,
                "prerequisite_failed",
            ));
            true
        } else {
            false
        }
    }

    pub fn retain_only(&mut self, only: &[String]) {
        if only.is_empty() {
            return;
        }

        self.findings.retain(|finding| {
            only.iter().any(|needle| {
                finding.id.starts_with(needle)
                    || finding.provider == needle
                    || finding.id.contains(needle)
            })
        });
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Severity {
    Ok,
    Info,
    Warn,
    Fail,
    Manual,
    Skipped,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Ok => "ok",
            Severity::Info => "info",
            Severity::Warn => "warn",
            Severity::Fail => "fail",
            Severity::Manual => "manual",
            Severity::Skipped => "skipped",
        }
    }
}

#[derive(Clone)]
pub struct Finding {
    pub id: &'static str,
    pub provider: &'static str,
    pub severity: Severity,
    pub status: &'static str,
    pub summary: String,
    pub evidence: String,
    pub cause: &'static str,
    pub repair: String,
}

impl Finding {
    pub fn ok(
        id: &'static str,
        provider: &'static str,
        status: &'static str,
        summary: &str,
        evidence: &str,
    ) -> Self {
        Self::new(
            id,
            provider,
            Severity::Ok,
            status,
            summary,
            evidence,
            "none",
            "",
        )
    }

    pub fn info(
        id: &'static str,
        provider: &'static str,
        status: &'static str,
        summary: &str,
        evidence: &str,
    ) -> Self {
        Self::new(
            id,
            provider,
            Severity::Info,
            status,
            summary,
            evidence,
            "none",
            "",
        )
    }

    pub fn warn(
        id: &'static str,
        provider: &'static str,
        status: &'static str,
        summary: &str,
        repair: &str,
    ) -> Self {
        Self::new(
            id,
            provider,
            Severity::Warn,
            status,
            summary,
            "",
            "attention_required",
            repair,
        )
    }

    pub fn fail(
        id: &'static str,
        provider: &'static str,
        status: &'static str,
        summary: &str,
        evidence: &str,
        cause: &'static str,
        repair: &str,
    ) -> Self {
        Self::new(
            id,
            provider,
            Severity::Fail,
            status,
            summary,
            evidence,
            cause,
            repair,
        )
    }

    pub fn manual(
        id: &'static str,
        provider: &'static str,
        status: &'static str,
        summary: &str,
        repair: &str,
    ) -> Self {
        Self::new(
            id,
            provider,
            Severity::Manual,
            status,
            summary,
            "",
            "manual_step_required",
            repair,
        )
    }

    pub fn skipped(
        id: &'static str,
        provider: &'static str,
        status: &'static str,
        summary: &str,
        cause: &'static str,
    ) -> Self {
        Self::new(
            id,
            provider,
            Severity::Skipped,
            status,
            summary,
            "",
            cause,
            "",
        )
    }

    fn new(
        id: &'static str,
        provider: &'static str,
        severity: Severity,
        status: &'static str,
        summary: &str,
        evidence: &str,
        cause: &'static str,
        repair: &str,
    ) -> Self {
        Self {
            id,
            provider,
            severity,
            status,
            summary: summary.to_string(),
            evidence: evidence.to_string(),
            cause,
            repair: repair.to_string(),
        }
    }
}

pub fn render_human_report(report: &Report) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "External World Report: {}", report.command);
    let _ = writeln!(out, "status: {}", if report.ok() { "ok" } else { "failed" });
    let _ = writeln!(out);

    for finding in &report.findings {
        let _ = writeln!(
            out,
            "[{}] {} ({})",
            finding.severity.as_str(),
            finding.id,
            finding.status
        );
        let _ = writeln!(out, "      {}", finding.summary);
        if !finding.evidence.is_empty() {
            let _ = writeln!(out, "      Evidence: {}", finding.evidence.trim());
        }
        if finding.cause != "none" {
            let _ = writeln!(out, "      Cause: {}", finding.cause);
        }
        if !finding.repair.is_empty() {
            let _ = writeln!(out, "      Repair: {}", finding.repair);
        }
    }

    out
}

pub fn render_json_report(report: &Report) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "{{\"command\":\"{}\",\"ok\":{},\"findings\":[",
        json_escape(report.command),
        report.ok()
    );

    for (idx, finding) in report.findings.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"id\":\"{}\",\"provider\":\"{}\",\"severity\":\"{}\",\"status\":\"{}\",\"summary\":\"{}\",\"evidence\":\"{}\",\"cause\":\"{}\",\"repair\":\"{}\"}}",
            json_escape(finding.id),
            json_escape(finding.provider),
            finding.severity.as_str(),
            json_escape(finding.status),
            json_escape(&finding.summary),
            json_escape(&finding.evidence),
            json_escape(finding.cause),
            json_escape(&finding.repair)
        );
    }

    out.push_str("]}");
    out
}

pub fn json_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_ok_is_false_only_for_failures() {
        let mut report = Report::new("test");
        report.findings.push(Finding::warn(
            "env.runtime",
            "env",
            "missing",
            "missing env",
            "set env",
        ));
        report.findings.push(Finding::manual(
            "provider.setup",
            "provider",
            "manual",
            "manual step",
            "open console",
        ));
        assert!(report.ok());

        report.findings.push(Finding::fail(
            "database.connect",
            "database",
            "failed",
            "connection failed",
            "refused",
            "unreachable",
            "start db",
        ));
        assert!(!report.ok());
    }

    #[test]
    fn json_renderer_escapes_control_characters() {
        let mut report = Report::new("test");
        report.findings.push(Finding::ok(
            "quoted",
            "local",
            "present",
            "contains \"quotes\" and\nnewlines",
            "path\\file",
        ));

        let json = render_json_report(&report);
        assert!(json.contains("\\\"quotes\\\""));
        assert!(json.contains("\\n"));
        assert!(json.contains("path\\\\file"));
    }

    #[test]
    fn only_filter_keeps_matching_ids_or_providers() {
        let mut report = Report::new("test");
        report
            .findings
            .push(Finding::ok("env.runtime", "env", "present", "env", ""));
        report.findings.push(Finding::ok(
            "database.migrations",
            "database",
            "present",
            "db",
            "",
        ));
        report.retain_only(&["database".to_string()]);

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].id, "database.migrations");
    }

    #[test]
    fn skip_if_failed_records_causal_skip() {
        let mut report = Report::new("test");
        report.findings.push(Finding::fail(
            "database.connect",
            "database",
            "failed",
            "connection failed",
            "",
            "unreachable",
            "fix db",
        ));

        assert!(report.skip_if_failed(
            "database.connect",
            "database.migrations",
            "database",
            "Skipping migrations because database connection failed.",
        ));
        assert_eq!(report.findings[1].severity, Severity::Skipped);
        assert_eq!(report.findings[1].cause, "prerequisite_failed");
    }
}
