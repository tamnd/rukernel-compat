# Security policy

## Supported versions

The project is before 1.0. Fixes go on the default branch only. After 1.0, this section will give a support period for each release.

## Report a vulnerability

Use the private vulnerability report of GitHub. Go to the Security tab of this repository and select "Report a vulnerability". Do not open a public issue for a problem that an attacker can use.

We send an answer in seven days or less. If you get no answer in seven days, write a public issue that asks for an answer. Do not describe the problem in that issue.

## What is in scope

The harness runs untrusted test programs and downloads test suites. These problems are in scope:

- A test program that can get out of its virtual machine and change the host.
- A suite download that the harness does not verify with its checksum.
- A credential or a token that the harness writes into a log or a result file.
- A path in a data file that makes the harness write outside its work directory.

A bug in rukernel itself goes to [tamnd/rukernel](https://github.com/tamnd/rukernel/security/advisories/new).

## What we do

We find the cause, make a fix on a private branch, and release the fix with an advisory. The advisory tells what the problem was and which versions it affects. We give credit to the reporter, unless the reporter does not want it.
