# os-trash-test (throwaway)

Answers issue #5: can the System Trash keep the Undo promise on each OS? It sends its own test items to the System Trash through a draft of each OS backend, restores them, and prints a result for each question.

It creates a new folder in your home folder and only trashes and removes what it creates there. It never empties the Trash. Outside that folder, it leaves any Trash folder the OS makes on first use and, on Windows, may remove a test item's own record from the Recycle Bin. It's not part of the app and comes out of `main` once ticket 01 is decided.

## Run it

From `prototypes/os-trash-test/`, on each OS, with Rust installed (Windows also needs the MSVC build tools):

```
cargo run -- --pause
```

To answer Q4, add a folder on a drive with no Trash. A new subfolder is made inside it and removed afterwards. Repeat the option to test a network drive and a removable drive in one run:

```
cargo run -- --pause --no-trash-volume /Volumes/share --no-trash-volume /Volumes/usb
cargo run -- --pause --no-trash-volume Z:\ --no-trash-volume E:\
cargo run -- --pause --no-trash-volume /mnt/share --no-trash-volume /media/you/usb
```

`--pause` stops once the items are trashed so you can look in the real Trash. Don't restore or delete them while it waits, and don't use the Trash for anything else during the run, as Windows's Clean restore check compares item counts.

Paste the whole output into the ticket. The `== Summary ==` block has the verdicts, and the lines above it say why, such as whether a no-Trash drive was spotted by the preflight or only when trashing was tried.

## Before you run it

- **macOS:** turn Full Disk Access off for your terminal (System Settings > Privacy & Security > Full Disk Access) and reopen the terminal. A terminal that has it can't show whether the app would need it, and the test reports that as `????`. The app is its own program to macOS, so a result from Terminal is a strong hint, not proof.
- **Linux:** the test folder is on the same disk as your home folder, so it uses the home Trash. Files on other disks use a Trash on that disk, created as `.Trash-<uid>` at its top if you have write access there, and it stays afterwards. No check here can tell if the desktop accepts these Trash entries, so use `--pause`, confirm the items are listed in the Trash, and say so in the ticket.

## Reading the results

| Question | Result means |
| --- | --- |
| Q1 Landing | The OS or backend named a location, and the item left its old path |
| Q2 Move back | A file, a folder and a Package each came back with the same contents |
| Q2 Full Disk Access (macOS) | Items came back from a terminal that can't list `~/.Trash` |
| Q2 Clean restore (Windows) | No `$I` record is left, and the Recycle Bin's item count is back where it started |
| Q2 Clean once removed (only if the above fails) | Deleting the leftover record by hand makes it clean |
| Q3 Same name | Two items with one name got separate locations and each came back with its own contents |
| Q4 No-Trash volume | The item was refused because there's no Trash, and is still in place |

`????` means the run couldn't answer that question, and its line says why. On Q4, the drive may have a Trash, or trashing failed for another reason. A macOS refusal only counts as "no Trash" if it's Foundation's `NSFeatureUnsupportedError`, so if a drive with no Trash gives `????`, its error code is what ticket 02 needs. If any drive or item gives `????`, the summary says NOT ANSWERED, even if others passed.

**Windows Q4:** a FAIL here destroys the test file, which is the point of the test. It means Windows permanently deleted an item without the backend being able to tell first. The `sink:` lines show the flags Windows passed. Compare those from the ordinary items with those from the no-Trash drive to see if `TSF_DELETE_RECYCLE_IF_POSSIBLE` (`0x80`) tells them apart.

**Linux Q4 on a real drive:** a drive you can write to at the top gets a `.Trash-<uid>`, so it counts as having a Trash. To test a drive with none, use a folder you own on a volume whose top you can't write to:

```
sudo mkdir -p /mnt/notrash && sudo mount -t tmpfs -o mode=755 tmpfs /mnt/notrash
sudo mkdir /mnt/notrash/mine && sudo chown "$USER" /mnt/notrash/mine
cargo run -- --no-trash-volume /mnt/notrash/mine
sudo umount /mnt/notrash
```

If items don't come back, the program lists where they are and leaves its folder in place to clear by hand.
