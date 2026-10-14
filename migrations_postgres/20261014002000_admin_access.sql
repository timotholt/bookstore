ALTER TABLE users ADD COLUMN account_status text NOT NULL DEFAULT 'active' CHECK(account_status IN ('active','disabled'));
ALTER TABLE users ADD COLUMN status_reason text NOT NULL DEFAULT '';
ALTER TABLE users ADD COLUMN status_changed_by text;
ALTER TABLE users ADD COLUMN status_changed_at timestamptz;
ALTER TABLE users ADD COLUMN profile_version bigint NOT NULL DEFAULT 0;
CREATE TABLE user_staff_roles(user_id text NOT NULL REFERENCES users(id) ON DELETE CASCADE, role text NOT NULL CHECK(role IN ('moderator','support','fulfillment','refund','administrator')), PRIMARY KEY(user_id,role));
INSERT INTO user_staff_roles SELECT user_id,'moderator' FROM review_moderators;
CREATE TABLE admin_audit(id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY, actor_id text, target_id text NOT NULL, action text NOT NULL, reason text NOT NULL, metadata jsonb NOT NULL DEFAULT '{}', created_at timestamptz NOT NULL DEFAULT now());
CREATE INDEX admin_audit_target ON admin_audit(target_id,id DESC);
CREATE INDEX users_admin_list ON users(created_at DESC,id);

DROP TABLE review_moderators;
CREATE FUNCTION prevent_account_delete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'Physical account deletion is unsupported; disable the account instead'; END $$;
CREATE TRIGGER users_disable_instead_of_delete BEFORE DELETE ON users FOR EACH ROW EXECUTE FUNCTION prevent_account_delete();
