#!/usr/bin/env python3
import pexpect
import os, sys, time

ip = "127.0.0.1"
port = "10000"
username = "nikitapekin@gmail.com"
password = "12345"

try:
    os.remove('../maildir/.lock')
except OSError:
    pass

child = pexpect.spawn('telnet '+ ip + ' ' + port)

child.expect('.\n')
child.logfile = sys.stdout.buffer
time.sleep(1)
child.sendline('1 login ' + username + ' ' + password)
child.expect('1 OK logged in successfully as nikitapekin@gmail.com')
child.sendline('2 select INBOX')
child.expect('successful')

#child.sendline('3 fetch 1:2 RFC822.SIZE')
#child.expect('completed')
#child.sendline('3 fetch 1:2 RFC822.HEADER')
#child.expect('completed')
#child.sendline('3 fetch 1:2 (RFC822.SIZE RFC822.HEADER)')
#child.expect('completed')

#child.sendline('3 fetch 1:3 ENVELOPE')
#child.expect('completed')

#child.sendline('a3 fetch 1,2,3 UID')
#child.expect('completed')

#child.sendline('3 fetch 1:3 INTERNALDATE')
#child.expect('completed')

#child.sendline('3 fetch 1:3 FLAGS')
#child.expect('completed')

child.sendline('3 fetch 1:3 BODY.PEEK[]')
child.expect('completed')

#child.sendline('3 fetch 1:2 (FLAGS BODY[HEADER.FIELDS (DATE FROM)])')
#child.expect('unimplemented')
