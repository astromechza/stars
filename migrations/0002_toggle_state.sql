-- Tri-state cells: a toggle row now carries a state.
--   1 = outline glow, 2 = full glow. Absence of a row = cleared (state 0).
-- Existing rows (binary "on") become state 1 (outline).
ALTER TABLE toggles ADD COLUMN state INTEGER NOT NULL DEFAULT 1;
