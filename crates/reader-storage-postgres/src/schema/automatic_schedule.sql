-- Shared deployment timing policy contains no tenant data. Every paid automatic
-- boundary and candidate claim checks the same clock; manual calls bypass it.
CREATE TABLE ai_automatic_schedule (
 singleton BOOLEAN PRIMARY KEY CHECK(singleton),
 pause_peak_hours BOOLEAN NOT NULL,
 calendar_valid_through DATE NOT NULL,
 holidays DATE[] NOT NULL
);
INSERT INTO ai_automatic_schedule VALUES(true,false,'2026-12-31',ARRAY[]::date[]);
-- NULL means allowed now; otherwise the exact first permissible execution time.
-- Missing/expired calendars fail closed for automatic calls, never for manual calls.
CREATE FUNCTION reader_ai_automatic_resume_at(instant TIMESTAMPTZ) RETURNS TIMESTAMPTZ LANGUAGE plpgsql STABLE AS $$
DECLARE policy ai_automatic_schedule%ROWTYPE;
BEGIN
 SELECT * INTO STRICT policy FROM ai_automatic_schedule WHERE singleton;
 RETURN CASE
 WHEN NOT policy.pause_peak_hours THEN NULL
 WHEN (instant AT TIME ZONE 'Asia/Shanghai')::date>policy.calendar_valid_through
 THEN (((instant AT TIME ZONE 'Asia/Shanghai')::date+1)::timestamp AT TIME ZONE 'Asia/Shanghai')
 WHEN extract(isodow FROM instant AT TIME ZONE 'Asia/Shanghai') IN (6,7)
 OR (instant AT TIME ZONE 'Asia/Shanghai')::date=ANY(policy.holidays) THEN NULL
 WHEN (instant AT TIME ZONE 'Asia/Shanghai')::time>='09:00'::time AND (instant AT TIME ZONE 'Asia/Shanghai')::time<'12:00'::time
 THEN (((instant AT TIME ZONE 'Asia/Shanghai')::date+'12:00'::time) AT TIME ZONE 'Asia/Shanghai')
 WHEN (instant AT TIME ZONE 'Asia/Shanghai')::time>='14:00'::time AND (instant AT TIME ZONE 'Asia/Shanghai')::time<'18:00'::time
 THEN (((instant AT TIME ZONE 'Asia/Shanghai')::date+'18:00'::time) AT TIME ZONE 'Asia/Shanghai')
 ELSE NULL END;
END
$$;

-- A peak may end between admission and lease release. Never turn that deferral
-- into a next-day budget delay: the automatic job is then runnable immediately.
CREATE FUNCTION reader_ai_deferred_until(peak BOOLEAN, instant TIMESTAMPTZ) RETURNS TIMESTAMPTZ LANGUAGE sql STABLE AS $$
 SELECT CASE WHEN peak THEN COALESCE(reader_ai_automatic_resume_at(instant),instant)
 ELSE ((instant AT TIME ZONE 'Europe/Moscow')::date+1)::timestamp AT TIME ZONE 'Europe/Moscow' END
$$;
