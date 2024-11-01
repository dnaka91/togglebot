SELECT kind, name, SUM(count) AS count FROM command_usage
GROUP BY year, month, kind, name
ORDER BY SUM(count) DESC;