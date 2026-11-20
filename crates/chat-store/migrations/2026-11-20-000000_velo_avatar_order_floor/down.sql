UPDATE OR IGNORE avatar_descriptors
SET jid = '__oxidezap_avatar_order_floor__'
WHERE jid = '__velo_avatar_order_floor__';
DELETE FROM avatar_descriptors WHERE jid = '__velo_avatar_order_floor__';
