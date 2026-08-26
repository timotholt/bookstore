ALTER TABLE users
    ADD COLUMN IF NOT EXISTS first_name TEXT,
    ADD COLUMN IF NOT EXISTS last_name TEXT,
    ADD COLUMN IF NOT EXISTS address_line1 TEXT,
    ADD COLUMN IF NOT EXISTS address_line2 TEXT,
    ADD COLUMN IF NOT EXISTS address_city TEXT,
    ADD COLUMN IF NOT EXISTS address_state TEXT,
    ADD COLUMN IF NOT EXISTS address_postal_code TEXT;

UPDATE users
SET
    first_name = NULLIF(split_part(full_name, ' ', 1), ''),
    last_name = NULLIF(regexp_replace(full_name, '^\S+\s*', ''), '')
WHERE full_name IS NOT NULL
  AND first_name IS NULL
  AND last_name IS NULL;
