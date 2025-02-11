SELECT kind AS "kind: CommandKind", name, SUM(count) AS "count: u32"
FROM command_usage
GROUP BY year, month, kind, name
ORDER BY SUM(count) DESC;