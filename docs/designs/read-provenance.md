# Read provenance

Schema17 records the method on the existing workspace/article/revision read event.
`reader` means an accepted focused-reading completion (rating or explicit abstention);
`single` means an individual read transition, including opening in the ordinary
reader; `bulk` means Mark all read. `unknown` represents old events whose method
was never recorded. A reader completion does not imply a numeric rating.

A new event requires an explicit method from this closed set. Article state,
ratings, events and completion receipts retain their existing transaction/undo
semantics. Undo preserves activity history and makes the article unread. A later
read transition gets its own method; idempotent retries create no extra events.
Bulk marking creates no rating. Existing events retain their exact identity and
timestamp during upgrade and receive unknown provenance, with no inference from
an unrelated historical rating.

Article DTOs expose nullable `readMethod`: unread articles have no current method.
A read article without an event is displayed as unknown. Ordinary owned lookups
select the latest revision's event within the same workspace; read history selects
the method of the latest event inside its explicit time interval. Ownership is
checked before these queries, and joins always include the workspace identity.
The UI keeps a permanent 20px metadata row across all read states.
