SELECT COALESCE(o.id, 0)::text AS result_id, p.status AS result_label FROM orders o RIGHT JOIN payments p ON p.order_id=o.id ORDER BY p.id;
