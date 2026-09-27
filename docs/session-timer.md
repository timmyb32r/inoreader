# Session countdown

The top bar shows an account-scoped, browser-local countdown. Default duration is
one hour; click the digits while stopped to set HH:MM:SS. The explicit supported
range is 00:00:01–99:59:59. Each start begins a separate session; this is not a
shared daily allowance and does not block reading.

Running state persists an absolute Unix-millisecond deadline. Pausing persists
the exact remaining milliseconds; resuming creates a new deadline. Reloading or
reopening the same browser restores that state, including time spent away.
Stopping returns to the selected duration. Storage errors are visible and prevent
starting an unsaved timer. Invalid stored state is not silently overwritten.
The selected duration remains available for subsequent starts in this browser.

At zero, the timer stops, plays a short synthesized tone and pulses four times.
Reduced-motion users receive a static highlight. No notification moves adjacent
controls. Browser Web Locks serialize completion between tabs and storage events
synchronize their displays. No server writes or external requests are involved.

Sound is unlocked by a user gesture. Closed pages cannot sound an alarm; browsers
may suspend background pages or block sound after reopening until interaction.
The stored deadline still ensures the correct remaining value on return. State
is not synchronized between different browsers or devices.

Tests cover input/persistence validation, exact paused state, wall-clock expiry,
reloads, audible completion without repeated alerts, and stable header geometry.
