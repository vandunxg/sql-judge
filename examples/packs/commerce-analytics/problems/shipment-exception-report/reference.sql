SELECT p.id AS result_id, p.name AS result_label FROM products p WHERE EXISTS (SELECT 1 FROM reviews r WHERE r.product_id=p.id AND r.score >= 4) ORDER BY p.id;
