#!/usr/bin/python3
"""Ajusta o spec do RPM para a versão de uma tag de release.

Uso:
    packaging/bump_spec.py 0.4.0 ["nota da versão"]

Reescreve a linha ``Version:`` e insere uma entrada no ``%changelog``.
Idempotente: se a versão já tem entrada no topo do changelog, não duplica.
"""

import datetime
import pathlib
import re
import sys

SPEC = pathlib.Path(__file__).with_name("dnfm.spec")
AUTHOR = "Joao Pedro G. M. S. <joaopedrogms@users.noreply.github.com>"


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print((__doc__ or "").strip(), file=sys.stderr)
        return 2

    version = argv[1].strip().lstrip("v")
    if not re.fullmatch(r"\d+(\.\d+)+", version):
        print(f"versão inválida: {argv[1]!r}", file=sys.stderr)
        return 2
    note = argv[2].strip() if len(argv) > 2 and argv[2].strip() else f"Versão {version}."

    texto = SPEC.read_text(encoding="utf-8")
    if not re.search(r"(?m)^Version:", texto):
        print("spec sem linha Version:", file=sys.stderr)
        return 1
    if "%changelog" not in texto:
        print("spec sem seção %changelog", file=sys.stderr)
        return 1

    novo = re.sub(r"(?m)^Version:.*$", f"Version:        {version}", texto, count=1)

    ja_existe = re.search(rf"(?m)^\* .* - {re.escape(version)}-\d+$", novo)
    if not ja_existe:
        agora = datetime.datetime.now(datetime.timezone.utc)
        data = f"{agora.strftime('%a %b')} {agora.day} {agora.year}"
        entrada = f"* {data} {AUTHOR} - {version}-1\n- {note}\n\n"
        novo = novo.replace("%changelog\n", "%changelog\n" + entrada, 1)

    if novo != texto:
        SPEC.write_text(novo, encoding="utf-8")
        print(f"spec ajustado para {version}")
    else:
        print(f"spec já estava em {version}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
