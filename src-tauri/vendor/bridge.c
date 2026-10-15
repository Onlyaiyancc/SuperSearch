#include <windows.h>
#include <wchar.h>

/* 命名绑定仅作用于本 SDK 工作线程，默认实例与原始供应商源文件保持不变。 */
static __declspec(thread) WCHAR supersearch_instance_class[256] = L"EVERYTHING_TASKBAR_NOTIFICATION";

BOOL __stdcall SuperSearch_InstanceWindowClass(LPCWSTR instance, LPWSTR output, DWORD capacity) {
    size_t length = instance ? wcslen(instance) : 0;
    LPCWSTR prefix = L"EVERYTHING_TASKBAR_NOTIFICATION";
    size_t prefix_length = wcslen(prefix);
    size_t required = prefix_length + (length ? length + 3 : 0) + 1;
    if (length > 128 || !output || capacity < required) return FALSE;
    wcscpy(output, prefix);
    if (length) { wcscat(output, L"_("); wcscat(output, instance); wcscat(output, L")"); }
    return TRUE;
}

BOOL __stdcall Everything_SetInstanceNameW(LPCWSTR instance) {
    return SuperSearch_InstanceWindowClass(instance, supersearch_instance_class, 256);
}

static HWND supersearch_find_window(LPCTSTR class_name, LPCTSTR title) {
    (void)class_name; (void)title;
    return FindWindowW(supersearch_instance_class, NULL);
}

/* Job 只拥有本应用启动的子进程；应用意外退出时系统关闭句柄自动清理。 */
HANDLE __stdcall SuperSearch_CreateEngineJob(void) {
    HANDLE job = CreateJobObjectW(NULL, NULL);
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION info = {0};
    if (!job) return NULL;
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if (!SetInformationJobObject(job, JobObjectExtendedLimitInformation, &info, sizeof(info))) { CloseHandle(job); return NULL; }
    return job;
}

BOOL __stdcall SuperSearch_AssignEngineJob(HANDLE job, HANDLE process) { return AssignProcessToJobObject(job, process); }

BOOL __stdcall SuperSearch_StopOwnedEngine(LPCWSTR instance, DWORD pid) {
    WCHAR class_name[256];
    DWORD actual_pid = 0;
    DWORD_PTR result = 0;
    HWND window;
    /* 空实例永远禁止退出；即使名称冲突也不能关闭别人的窗口。 */
    if (!instance || !instance[0] || !pid || !SuperSearch_InstanceWindowClass(instance, class_name, 256)) return FALSE;
    window = FindWindowW(class_name, NULL);
    if (!window) return FALSE;
    GetWindowThreadProcessId(window, &actual_pid);
    if (actual_pid != pid) return FALSE;
    return SendMessageTimeoutW(window, WM_USER, 4, 0, SMTO_ABORTIFHUNG | SMTO_BLOCK | SMTO_ERRORONEXIT, 100, &result) != 0;
}

/* SDK 的所有同步 IPC 发送共享当前事务截止时间，防止无响应客户端阻塞工作线程。 */
static ULONGLONG supersearch_deadline = 0;
static BOOL supersearch_ipc_failed = FALSE;

void __stdcall SuperSearch_SetIpcTimeout(DWORD budget_ms) {
    supersearch_deadline = GetTickCount64() + budget_ms;
    supersearch_ipc_failed = FALSE;
}

BOOL __stdcall SuperSearch_IpcFailed(void) { return supersearch_ipc_failed; }

static LRESULT supersearch_send_message(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    DWORD_PTR result = 0;
    ULONGLONG now = GetTickCount64();
    if (now >= supersearch_deadline) { supersearch_ipc_failed = TRUE; return 0; }
    DWORD budget = (DWORD)(supersearch_deadline - now);
    if (budget > 40) budget = 40;
    if (!SendMessageTimeoutW(window, message, wparam, lparam,
            SMTO_ABORTIFHUNG | SMTO_BLOCK | SMTO_ERRORONEXIT, budget, &result)) {
        supersearch_ipc_failed = TRUE;
        return 0;
    }
    return (LRESULT)result;
}

#undef SendMessage
#define SendMessage supersearch_send_message
#undef FindWindow
#define FindWindow supersearch_find_window
#include "Everything.c"
#undef FindWindow

/* 只读 SCM 状态；专属服务以原厂签名程序提供卷读取权限。 */
BOOL __stdcall SuperSearch_FastServiceRunning(void) {
    SERVICE_STATUS status;
    BOOL running = FALSE;
    SC_HANDLE manager = OpenSCManagerW(NULL, NULL, SC_MANAGER_CONNECT);
    if (!manager) return FALSE;
    SC_HANDLE service = OpenServiceW(manager, L"Everything (SuperSearch)", SERVICE_QUERY_STATUS);
    if (service) {
        if (QueryServiceStatus(service, &status)) running = status.dwCurrentState == SERVICE_RUNNING;
        CloseServiceHandle(service);
    }
    CloseServiceHandle(manager);
    return running;
}

BOOL __stdcall SuperSearch_ReparseTagRedirects(DWORD tag) {
    return IsReparseTagNameSurrogate(tag) != 0;
}

/* 云占位符也有重解析属性；只有名字重定向才需要保留目录索引。 */
BOOL __stdcall SuperSearch_DirectoryRedirects(const WCHAR *path) {
    WIN32_FIND_DATAW data;
    HANDLE search = FindFirstFileW(path, &data);
    if (search == INVALID_HANDLE_VALUE) return TRUE;
    FindClose(search);
    return (data.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT)
        && SuperSearch_ReparseTagRedirects(data.dwReserved0);
}

static DWORD supersearch_read_dword(const BYTE *data) {
    DWORD value;
    CopyMemory(&value, data, sizeof(value));
    return value;
}

static BOOL supersearch_utf16_string(const BYTE *data, DWORD size, DWORD position, DWORD *length) {
    DWORD count = 0;
    if (position > size || (position & 1)) return FALSE;
    while (position <= size && size - position >= 2 && count <= 32767) {
        WORD value;
        CopyMemory(&value, data + position, sizeof(value));
        if (!value) { *length = count; return TRUE; }
        position += 2;
        count++;
    }
    return FALSE;
}

/* IPC 是本地进程输入边界。SDK 复制和访问之前核验全部大小、偏移和 UTF-16 终止符。 */
BOOL __stdcall SuperSearch_ValidatePayload(const BYTE *data, DWORD size, DWORD version, DWORD offset, DWORD maximum) {
    DWORD count, total, table_end, i;
    if (!data || size > 16 * 1024 * 1024 || offset >= 1000 || maximum > 200) return FALSE;
    if (version == 1) {
        if (size < 28) return FALSE;
        total = supersearch_read_dword(data + 8);
        count = supersearch_read_dword(data + 20);
        if (count > maximum || count > 200 || count > total || supersearch_read_dword(data + 24) != offset) return FALSE;
        if ((ULONGLONG)supersearch_read_dword(data) + supersearch_read_dword(data + 4) != total) return FALSE;
        if ((ULONGLONG)supersearch_read_dword(data + 12) + supersearch_read_dword(data + 16) != count) return FALSE;
        table_end = 28 + count * 12;
        if (size < table_end) return FALSE;
        for (i = 0; i < count; i++) {
            DWORD name_length, path_length;
            DWORD position = 28 + i * 12;
            DWORD flags = supersearch_read_dword(data + position);
            DWORD name_offset = supersearch_read_dword(data + position + 4);
            DWORD path_offset = supersearch_read_dword(data + position + 8);
            if (flags & ~3u || name_offset < table_end || path_offset < table_end) return FALSE;
            if (!supersearch_utf16_string(data, size, name_offset, &name_length) || !supersearch_utf16_string(data, size, path_offset, &path_length)) return FALSE;
            if ((ULONGLONG)name_length + path_length + 1 > 32767) return FALSE;
        }
        return TRUE;
    }
    if (version == 2) {
        if (size < 20) return FALSE;
        total = supersearch_read_dword(data);
        count = supersearch_read_dword(data + 4);
        if (count > maximum || count > 200 || count > total || supersearch_read_dword(data + 8) != offset) return FALSE;
        if (supersearch_read_dword(data + 12) != 3 || supersearch_read_dword(data + 16) != 1) return FALSE;
        table_end = 20 + count * 8;
        if (size < table_end) return FALSE;
        for (i = 0; i < count; i++) {
            DWORD position = supersearch_read_dword(data + 20 + i * 8 + 4);
            DWORD combined_length = 0, field;
            if (supersearch_read_dword(data + 20 + i * 8) & ~3u || position < table_end || (position & 1)) return FALSE;
            for (field = 0; field < 2; field++) {
                DWORD expected_length, actual_length;
                if (position > size || size - position < 4) return FALSE;
                expected_length = supersearch_read_dword(data + position);
                position += 4;
                if (expected_length > 32767 || !supersearch_utf16_string(data, size, position, &actual_length) || actual_length != expected_length) return FALSE;
                combined_length += actual_length;
                position += (actual_length + 1) * 2;
            }
            if (combined_length + 1 > 32767) return FALSE;
        }
        return TRUE;
    }
    return FALSE;
}

BOOL __stdcall SuperSearch_ValidateReply(LPARAM parameter, DWORD reply_id) {
    COPYDATASTRUCT *reply = (COPYDATASTRUCT *)parameter;
    if (!reply || reply->dwData != reply_id || !reply->lpData) return FALSE;
    return SuperSearch_ValidatePayload((const BYTE *)reply->lpData, reply->cbData, _Everything_QueryVersion, _Everything_Offset, _Everything_Max);
}
