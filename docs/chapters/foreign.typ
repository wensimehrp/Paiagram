= Loading Files

Paiagram supports loading from these file formats:

- Native `.paiagram`,
- qETRC/pyETRC `.pyetgr`,
- OuDiaSecond `.oud2`,
- OpenTTD JGRPP `.json` timetable exports,
- GTFS `.zip`. Note that you must pass in a zip archive that includes all information.

You can also import files of other file formats by using a custom JavaScript import script.

== Using the CLI

You can use the CLI (command line interface) to import files:

```sh
$ paiagram -o <YOUR FILE>
```

This works for all file formats except OpenTTD JGRPP timetable exports. For timetable export files, you muse use the
`--jgrpp` command line argument:

```sh
$ paiagram --jgrpp ~/.local/share/openttd/orderlist/*.json
```

You could import multiple timetable exports by specifying multiple paths, or using the `*` syntax. In the example above,
Paiagram reads all `.json` files from the default orderlist export location.

== Using the GUI

You can also import from the GUI.

== Nuances

=== qETRC/pyETRC

Paiagram does not retain the classes of trains

=== OuDiaSecond

Paiagram does not keep station actions, and won't create new vehicles.

=== JGRPP orderlists

Paiagram does not keep tags, conditional orders, "go to nearest depot" commands, or load specifications. Orderlists containing conditionals will be omitted when importing.

You would have to do manual wiring and connect stations manually.
