#!/usr/bin/python3
"""Captura payloads reais do dnf5daemon para servirem de fixture nos testes.

Roda como usuario normal, so faz leitura + resolve (nunca do_transaction).
Uso:  scripts/capture_fixtures.py [destino]
"""
import json
import os
import sys
from gi.repository import Gio, GLib

BUS, ROOT = "org.rpm.dnf.v0", "/org/rpm/dnf/v0"
DEST = sys.argv[1] if len(sys.argv) > 1 else "crates/dnfm-core/tests/fixtures"

conn = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)


def call(iface, method, params=None, path=ROOT):
    return conn.call_sync(BUS, path, iface, method, params, None,
                          Gio.DBusCallFlags.NONE, 240000, None)


def opts(d):
    return GLib.Variant('(a{sv})', ({k: GLib.Variant(t, v) for k, (t, v) in d.items()},))


def plain(x):
    """GLib.Variant -> estrutura Python serializavel."""
    if isinstance(x, GLib.Variant):
        return plain(x.unpack())
    if isinstance(x, dict):
        return {str(k): plain(v) for k, v in x.items()}
    if isinstance(x, (list, tuple)):
        return [plain(v) for v in x]
    return x


def dump(name, obj):
    os.makedirs(DEST, exist_ok=True)
    path = os.path.join(DEST, name)
    with open(path, "w") as fh:
        json.dump(obj, fh, indent=2, ensure_ascii=False, default=str)
    n = len(obj) if isinstance(obj, list) else 1
    print(f"  {name}: {n} entradas -> {os.path.getsize(path)} bytes")


sess = call("org.rpm.dnf.v0.SessionManager", "open_session",
            opts({"load_available_repos": ("b", True)})).unpack()[0]
print("sessao aberta")

INSTALLED_ATTRS = ["name", "evr", "epoch", "version", "release", "arch", "repo_id",
                   "install_size", "install_time", "summary", "reason", "nevra", "full_nevra"]
inst = call("org.rpm.dnf.v0.rpm.Rpm", "list",
            opts({"scope": ("s", "installed"), "package_attrs": ("as", INSTALLED_ATTRS)}),
            path=sess).unpack()[0]
print(f"instalados: {len(inst)}")
dump("installed.json", plain(inst[:5]))
dump("installed_full_count.json", [len(inst)])

ups = call("org.rpm.dnf.v0.rpm.Rpm", "list",
           opts({"scope": ("s", "upgradable"),
                 "package_attrs": ("as", INSTALLED_ATTRS + ["download_size", "from_repo_id"])}),
           path=sess).unpack()[0]
print(f"upgradable: {len(ups)}")
dump("upgradable.json", plain(ups[:5]))
dump("upgradable_full_count.json", [len(ups)])

adv = call("org.rpm.dnf.v0.Advisory", "list",
           opts({"advisory_attrs": ("as", ["advisoryid", "type", "severity", "title",
                                           "description", "collections"]),
                 "availability": ("s", "available"), "limit": ("i", 5)}),
           path=sess).unpack()[0]
print(f"advisories: {len(adv)}")
_adv = plain(adv)[:2]
for a in _adv:
    if isinstance(a.get("description"), str):
        a["description"] = a["description"][:120]
dump("advisories.json", _adv)

hist = None
for sig in ("x", "u", "i"):
    try:
        hist = call("org.rpm.dnf.v0.History", "list",
                    opts({"limit": (sig, 3), "package_attrs": ("as", ["name", "evr", "arch"])}),
                    path=sess).unpack()[0]
        print(f"history: limit assinatura '{sig}' aceita")
        break
    except GLib.Error as exc:
        print(f"history limit '{sig}': {exc.message[:80]}")
if hist is None:
    hist = call("org.rpm.dnf.v0.History", "list",
                opts({"package_attrs": ("as", ["name", "evr", "arch"])}), path=sess).unpack()[0]
    hist = hist[:3]
print(f"history: {len(hist)}")
dump("history.json", plain(hist))

repos = call("org.rpm.dnf.v0.rpm.Repo", "list",
             opts({"repo_attrs": ("as", ["id", "name", "enabled", "baseurl", "metalink"])}),
             path=sess).unpack()[0]
print(f"repos: {len(repos)}")
dump("repos.json", plain(repos))

# preview de remocao de um pacote folha conhecido
target = sys.argv[2] if len(sys.argv) > 2 else "dosbox-staging"
call("org.rpm.dnf.v0.rpm.Rpm", "remove",
     GLib.Variant('(asa{sv})', ([target], {})), path=sess)
items, result = call("org.rpm.dnf.v0.Goal", "resolve", opts({}), path=sess).unpack()
probs = call("org.rpm.dnf.v0.Goal", "get_transaction_problems_string", None, sess).unpack()[0]
dump("preview_remove.json", {"target": target, "result": result,
                             "items": plain(items), "problems": plain(probs)})
call("org.rpm.dnf.v0.Goal", "reset", None, sess)

# preview que deve falhar: pacote critico
call("org.rpm.dnf.v0.rpm.Rpm", "remove", GLib.Variant('(asa{sv})', (["glibc"], {})), path=sess)
items2, result2 = call("org.rpm.dnf.v0.Goal", "resolve", opts({}), path=sess).unpack()
probs2 = call("org.rpm.dnf.v0.Goal", "get_transaction_problems_string", None, sess).unpack()[0]
dump("preview_blocked.json", {"target": "glibc", "result": result2,
                              "items": plain(items2), "problems": plain(probs2)})
call("org.rpm.dnf.v0.Goal", "reset", None, sess)

call("org.rpm.dnf.v0.SessionManager", "close_session", GLib.Variant('(o)', (sess,)))
print("pronto (nada foi executado)")
