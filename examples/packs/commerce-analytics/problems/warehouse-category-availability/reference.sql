SELECT p.id AS result_id, p.name AS result_label FROM products p WHERE p.active AND p.list_price BETWEEN 50 AND 500 ORDER BY p.list_price DESC, p.id FETCH FIRST 50 ROWS ONLY;
