SELECT i.id AS result_id, p.name AS result_label FROM order_items i INNER JOIN products p ON p.id=i.product_id ORDER BY i.id FETCH FIRST 100 ROWS ONLY;
