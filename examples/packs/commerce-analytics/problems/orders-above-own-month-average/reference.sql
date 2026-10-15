SELECT p.id AS result_id, p.name AS result_label FROM products p JOIN inventory i ON i.product_id=p.id GROUP BY p.id, p.name HAVING SUM(i.quantity) < 30 ORDER BY p.id;
