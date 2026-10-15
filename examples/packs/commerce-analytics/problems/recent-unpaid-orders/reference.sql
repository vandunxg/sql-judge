SELECT c.id AS result_id, c.display_name AS result_label FROM customers c LEFT JOIN orders o ON o.customer_id = c.id GROUP BY c.id, c.display_name HAVING COUNT(o.id) = 0 ORDER BY c.id;
