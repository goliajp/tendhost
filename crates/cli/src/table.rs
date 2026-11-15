/// Rows as left-aligned columns under a header, two spaces apart. An
/// absent value is shown as `—`.
pub fn render(header: &[&str], rows: &[Vec<Option<String>>]) -> String {
    let cell = |value: &Option<String>| {
        value
            .clone()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "—".to_string())
    };
    let rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| row.iter().map(cell).collect())
        .collect();
    let widths: Vec<usize> = (0..header.len())
        .map(|i| {
            rows.iter()
                .map(|row| row[i].chars().count())
                .chain([header[i].chars().count()])
                .max()
                .unwrap_or(0)
        })
        .collect();
    let line = |cells: Vec<&str>| {
        let padded: Vec<String> = cells
            .iter()
            .zip(&widths)
            .map(|(cell, width)| format!("{cell}{}", " ".repeat(width - cell.chars().count())))
            .collect();
        padded.join("  ").trim_end().to_string()
    };
    let mut out = vec![line(header.to_vec())];
    out.extend(
        rows.iter()
            .map(|row| line(row.iter().map(String::as_str).collect())),
    );
    out.join("\n") + "\n"
}

/// Bytes in the largest unit that keeps the number at least 1.
pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_line_up_and_absent_values_show_as_a_dash() {
        let rows = vec![
            vec![Some("db-01".into()), Some("online".into()), None],
            vec![Some("a".into()), Some("offline".into()), Some("web".into())],
        ];
        let out = render(&["NAME", "STATE", "LABELS"], &rows);
        assert_eq!(
            out,
            "NAME   STATE    LABELS\ndb-01  online   —\na      offline  web\n"
        );
    }

    #[test]
    fn an_empty_table_is_just_its_header() {
        assert_eq!(render(&["NAME"], &[]), "NAME\n");
    }

    #[test]
    fn bytes_read_in_a_fitting_unit() {
        assert_eq!(
            [bytes(512), bytes(1536), bytes(64 << 30)],
            ["512 B", "1.5 KB", "64.0 GB"]
        );
    }
}
