author:	ds84182
association:	none
edited:	false
status:	none
--
This isn't any different than #62958, which is still open, so of course this would occur here. `good_delegate` is a no-op, and the chain of bad delegates is just a symptom of #62958, as each produced future is the payload for another future.
--
