SEGIMAP
=======

This is an IMAP server implementation written in Rust as a class project.
There is also an LMTP server attached so that SMTP servers may deliver mail without modifying the maildir themselves.

Some notes about Rust
---------------------

The most confusing thing for those reading the code who are unfamiliar with rust will likely be &str and String. &str is an actual string while String is actually a StringBuffer. String.as_slice() is used to get a &str out of a String and &str.to_string() is used to get a String out of a &str. Sometimes we need something which is neither of these to be a string so we use .as_slice() and .to_string() as appropriate. Sometimes we need a String but the thing we have can only be converted to &str so we have to chain the calls like so: .as_slice().to_string(). It sometimes also occurs the other way around.

& denotes a pointer (ie: pass-by-reference semantics). When you see &mut it means that the pointer is mutable. Rust only allows one mutable pointer at a time and enforces this at compile time. However, multiple immutable pointers may be created. * is used to dereference a pointer. In most cases, method calls will automatically dereference when needed.

The statement `return thing;` is equivalent to `thing` (note the absence of a semi-colon).

That should be everything someone who doesn't know rust needs to understand this code and why we do things certain ways.

Some notes about IMAP
---------------------

IMAP has three states: unauthenticated (before log in), authenticated and selected. The most important commands for actually reading mail are log in, to get to an authenticated state; list, to get the list of folders; select, to get to the selected state; fetch, to get data and metadata about the messages; and store, to modify the flags (metadata) of the messages.

### Relevant RFCS:

[RFC 3501 - IMAP4rev1](http://tools.ietf.org/html/rfc3501)  
[RFC 2822 - Internet message format](http://tools.ietf.org/html/rfc2822)  
[RFC 2033 - LMTP](http://tools.ietf.org/html/rfc2033)  
[RFC 2821 - SMTP (LMTP is based heavily on SMTP) ](http://tools.ietf.org/html/rfc2821)  
[RFC 2045 - MIME Part 1](http://tools.ietf.org/html/rfc2045)  
[RFC 2046 - MIME Part 2](http://tools.ietf.org/html/rfc2046)  

Installing, building, running
-----------------------------

Grab rust v0.12  
Grab cargo  
Run `cargo run` (alternatively, if you just want to compile the program, run `cargo build`)  
