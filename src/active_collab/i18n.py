import os
from collections.abc import Mapping

_CATALOG: dict[str, dict[str, str]] = {
    "pt_BR": {
        # render.py labels
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
        # cli.py runtime messages
        "No instances configured. Run: active_collab.py setup add": (
            "Nenhuma instância configurada. Execute: active_collab.py setup add"
        ),
        "Error: no instances configured. Run: active_collab.py setup add": (
            "Erro: nenhuma instância configurada. Execute: active_collab.py setup add"
        ),
        "Error: no instances configured. Run: active-collab setup add": (
            "Erro: nenhuma instância configurada. Execute: active-collab setup add"
        ),
        "Error: instance '{name}' not found. Known: {known}": (
            "Erro: instância '{name}' não encontrada. Conhecidas: {known}"
        ),
        "Error: multiple instances configured ({names}). Use --instance NAME.": (
            "Erro: múltiplas instâncias configuradas ({names}). Use --instance NOME."
        ),
        "Error: multiple instances ({names}). Use --instance NAME.": (
            "Erro: múltiplas instâncias ({names}). Use --instance NOME."
        ),
        "Error: cannot parse task ref '{ref}'. Use URL or PROJECT_ID/TASK_ID (e.g. 665/75159).": (
            "Erro: não foi possível interpretar a referência de tarefa '{ref}'."
            " Use URL ou PROJECT_ID/TASK_ID (ex: 665/75159)."
        ),
        "Error: task {p}/{t} not found (HTTP {status}).": (
            "Erro: tarefa {p}/{t} não encontrada (HTTP {status})."
        ),
        "Error: --name, --url and --email are required.": (
            "Erro: --name, --url e --email são obrigatórios."
        ),
        "Error: password is required.": "Erro: a senha é obrigatória.",
        "Error: {detail}": "Erro: {detail}",
        "Error: instance '{name}' not found.": "Erro: instância '{name}' não encontrada.",
        "Error: not in a git repository or HEAD is detached.": (
            "Erro: não está em um repositório git ou HEAD está desanexado."
        ),
        "Error: branch '{branch}' does not match expected pattern"
        " (feature|hotfix|fix)/PROJECT_ID-TASK_ID (e.g. feature/665-75159).": (
            "Erro: branch '{branch}' não corresponde ao padrão esperado"
            " (feature|hotfix|fix)/PROJECT_ID-TASK_ID (ex: feature/665-75159)."
        ),
        "No open tasks assigned to you.": "Nenhuma tarefa aberta atribuída a você.",
        "Connectivity: OK": "Conectividade: OK",
        "Connectivity: FAILED (HTTP {status})": "Conectividade: FALHOU (HTTP {status})",
        "Connectivity: FAILED ({exc})": "Conectividade: FALHOU ({exc})",
        "Instance '{name}' saved.": "Instância '{name}' salva.",
        "Instance '{name}' removed.": "Instância '{name}' removida.",
        "OK ({status})": "OK ({status})",
        "FAILED (HTTP {status})": "FALHOU (HTTP {status})",
        "FAILED ({exc})": "FALHOU ({exc})",
        # tui.py screen titles and messages
        "Projects": "Projetos",
        "Tasks": "Tarefas",
        "My Open Tasks": "Minhas Tarefas Abertas",
        "Assets": "Anexos",
        "Branch type (Enter to confirm, q cancel)": (
            "Tipo de branch (Enter para confirmar, q cancelar)"
        ),
        "Terminal too small": "Terminal muito pequeno",
        "Resize to continue": "Redimensione para continuar",
        "Press any key...": "Pressione qualquer tecla...",
        "Press any key to exit...": "Pressione qualquer tecla para sair...",
        "Downloaded:": "Baixado:",
        "Error:": "Erro:",
        # _hint_bar action labels
        "move": "mover",
        "select": "selecionar",
        "quit": "sair",
        "back": "voltar",
        "branch": "branch",
        "assets": "anexos",
        "scroll": "rolar",
        "page": "página",
        "open": "abrir",
        "download": "baixar",
        # detail-view redesign labels
        "Details": "Detalhes",
        "Artifacts": "Anexos/Artefatos",
        "Error: 'browse' requires an interactive terminal (TTY).": (
            "Erro: 'browse' requer um terminal interativo (TTY)."
        ),
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
