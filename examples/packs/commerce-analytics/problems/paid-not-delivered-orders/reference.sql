SELECT p.id AS result_id, p.name AS result_label FROM products p WHERE p.list_price >= ALL (SELECT peer.list_price FROM products peer WHERE peer.category_id=p.category_id) ORDER BY p.id;
