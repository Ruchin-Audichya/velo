-- The avatar order floor is a sentinel row keyed by a fixed jid, and 1.1
-- renames that jid with the rest of Velo's pre-1.1 names. Carry each
-- device's existing floor over so compacted removals stay protected.
UPDATE OR IGNORE avatar_descriptors
SET jid = '__velo_avatar_order_floor__'
WHERE jid = '__oxidezap_avatar_order_floor__';
DELETE FROM avatar_descriptors WHERE jid = '__oxidezap_avatar_order_floor__';
