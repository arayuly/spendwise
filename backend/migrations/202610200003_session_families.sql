-- A device session retains its family across rotation, including during logout races.
ALTER TABLE refresh_sessions ADD COLUMN family_id UUID;
UPDATE refresh_sessions SET family_id=id;
ALTER TABLE refresh_sessions ALTER COLUMN family_id SET NOT NULL;
CREATE INDEX refresh_sessions_family_idx ON refresh_sessions(user_id,family_id);
