
-- Apply predatory hunting to all Beasts in the Cat family (family 2) 
-- but only if they don't already have a specialized script.
UPDATE creature_template SET ScriptName = 'npc_predator_cat' 
WHERE family = 2 AND ScriptName = '';
