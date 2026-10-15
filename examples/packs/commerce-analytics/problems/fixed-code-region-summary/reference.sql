SELECT region_code::text AS result_id, name AS result_label FROM regions NATURAL JOIN regional_targets ORDER BY region_code;
