import os
from collections.abc import Mapping

_CATALOG: dict[str, dict[str, str]] = {
    "pt_BR": {
        "Task": "Tarefa",
        "Name": "Nome",
        "Status": "Status",
        "Assignee": "Responsável",
        "Start": "Início",
        "Due": "Prazo",
        "Estimate": "Estimativa",
        "Logged": "Registrado",
        "Description": "Descrição",
        "(no description)": "(sem descrição)",
        "(unassigned)": "(não atribuído)",
        "(unknown)": "(desconhecido)",
        "Comments": "Comentários",
        "Completed": "Concluído",
        "Open": "Aberto",
        "INSTANCE": "INSTÂNCIA",
        "PROJECT": "PROJETO",
        "TASK#": "TAREFA#",
        "TASK_ID": "ID_TAREFA",
        "NAME": "NOME",
    }
}

_lang: str = "en"


def _resolve_lang(env: Mapping[str, str]) -> str:
    for key in ("ACTIVE_COLLAB_LANG", "LANG", "LC_ALL"):
        raw = env.get(key, "")
        if not raw:
            continue
        normalized = _normalize_locale(raw)
        if normalized:
            return normalized
    return "en"


def _normalize_locale(raw: str) -> str:
    locale = raw.split(".")[0]
    if locale == "pt_BR":
        return "pt_BR"
    return ""


def set_language(lang: str) -> None:
    global _lang
    _lang = lang


def __(s: str) -> str:
    return _CATALOG.get(_lang, {}).get(s, s)


_lang = _resolve_lang(os.environ)
