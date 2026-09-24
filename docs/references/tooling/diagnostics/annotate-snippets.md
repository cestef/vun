author:	Muscraft
association:	member
edited:	false
status:	commented
--
> the main problem I faced is that there is no difference on an tests output? and I don't know if there should be any difference, so yeah, maybe I missed something or maybe it should be like this

There really shouldn't be any test difference as `annotate-snippets` is already used on nightly, which means that all tests are currently ran against it.

*[View changes since this review](https://triagebot.infra.rust-lang.org/gh-changes-since/rust-lang/rust/150032/0160933b1dd9547d46542b0ceedda00c37890c6d..9932c0ba67ca22db5736cd19cf38be8519a63783)*
--
author:	rustbot
association:	collaborator
edited:	false
status:	none
--
`rustc_errors::annotate_snippet_emitter_writer` was changed

cc @Muscraft

`rustc_errors::emitter` was changed

cc @Muscraft

Some changes occurred in src/tools/clippy

cc @rust-lang/clippy
--
author:	rustbot
association:	collaborator
edited:	false
status:	none
--
r? @JonathanBrouwer

rustbot has assigned @JonathanBrouwer.
They will have a look at your PR within the next two weeks and either review your PR or reassign to another reviewer.

Use `r?` to explicitly pick a reviewer
--
author:	Kivooeo
association:	member
edited:	false
status:	none
--
@Muscraft, feel free to assign yourself on this

I also took a look if documentation should be adjusted for this change and has found nothing related, so it seems like this is the only needed change to close tracking issue
--
author:	JonathanBrouwer
association:	member
edited:	false
status:	none
--
I'm happy to review this, but Muscraft seems like the better option here.
r? @Muscraft 
If you don't have capacity to review this or any other reason feel free to assign this back to me :)
--
author:	Muscraft
association:	member
edited:	false
status:	approved
--

--
author:	Muscraft
association:	member
edited:	false
status:	none
--
@bors r+ rollup
--
author:	bors
association:	collaborator
edited:	false
status:	none
--
:pushpin: Commit 84f2854bc3bfea59aeb6fd23245a788196c9d4ff has been approved by `Muscraft`

It is now in the [queue](https://bors.rust-lang.org/homu/queue/rust) for this repository.

<!-- @bors r=Muscraft 84f2854bc3bfea59aeb6fd23245a788196c9d4ff -->
<!-- homu: {"type":"Approved","sha":"84f2854bc3bfea59aeb6fd23245a788196c9d4ff","approver":"Muscraft","queue":"https://bors.rust-lang.org/homu/queue/rust"} -->
--
