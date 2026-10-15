SELECT e.id AS result_id, e.name AS result_label FROM employees e LEFT JOIN employees m ON m.id=e.manager_id WHERE e.manager_id IS NOT NULL ORDER BY e.id;
