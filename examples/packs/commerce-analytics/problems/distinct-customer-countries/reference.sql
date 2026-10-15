SELECT DISTINCT ON (a.customer_id) a.id AS result_id, a.line1 AS result_label FROM customer_addresses a ORDER BY a.customer_id, a.updated_at DESC, a.id;
