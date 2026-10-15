SELECT c.id AS result_id, c.display_name AS result_label FROM customers c WHERE c.active AND c.country_code IN ('US','CA') ORDER BY c.id FETCH FIRST 50 ROWS ONLY;
