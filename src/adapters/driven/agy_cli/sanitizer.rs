//! Response sanitization, intermediate log stripping, and status filtering for Antigravity CLI output.

/// Sanitizes Antigravity CLI agent output by stripping out intermediate tool-waiting
/// logs, <SYSTEM_MESSAGE> blocks, and background task status lines that leak into multi-step JSON responses.
pub fn sanitize_agent_response(raw: &str) -> String {
    // 1. First, strip multi-line <SYSTEM_MESSAGE> blocks and system headers
    let stripped_system_blocks = strip_system_message_blocks(raw);

    let mut cleaned_lines = Vec::new();
    let mut skipping_header = true;

    for line in stripped_system_blocks.lines() {
        let trimmed = line.trim();

        let is_intermediate = is_intermediate_agent_status(trimmed);

        if skipping_header && is_intermediate {
            // Drop intermediate tool progress status at the beginning of the response
            continue;
        }

        if !trimmed.is_empty() && !is_intermediate {
            skipping_header = false;
        }

        if !is_intermediate {
            cleaned_lines.push(line);
        }
    }

    let result = cleaned_lines.join("\n").trim().to_string();
    if result.is_empty() {
        raw.trim().to_string()
    } else {
        result
    }
}

pub fn strip_system_message_blocks(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut cursor = 0;

    while cursor < text.len() {
        let slice = &text[cursor..];

        // Identify starting point of system message block
        let start_offset = if let Some(pos) = slice.find("The following is a <SYSTEM_MESSAGE>") {
            Some(pos)
        } else if let Some(pos) = slice.find("<SYSTEM_MESSAGE>") {
            Some(pos)
        } else if let Some(pos) = slice.find("<SYSTEM_MESSAGE") {
            Some(pos)
        } else {
            None
        };

        if let Some(start_pos) = start_offset {
            let abs_start = cursor + start_pos;
            result.push_str(&text[cursor..abs_start]);

            let after_start = &text[abs_start..];
            // Look for matching end tag: </SYSTEM_MESSAGE>} or </SYSTEM_MESSAGE>
            let end_offset = if let Some(pos) = after_start.find("</SYSTEM_MESSAGE>}") {
                Some(pos + "</SYSTEM_MESSAGE>}".len())
            } else if let Some(pos) = after_start.find("</SYSTEM_MESSAGE>") {
                Some(pos + "</SYSTEM_MESSAGE>".len())
            } else {
                None
            };

            if let Some(end_len) = end_offset {
                cursor = abs_start + end_len;
            } else {
                // If no closing tag found, skip the rest of this system message block
                break;
            }
        } else {
            result.push_str(slice);
            break;
        }
    }

    result
}

pub fn is_intermediate_agent_status(line: &str) -> bool {
    let lower = line.to_lowercase();

    // 1. Task waiting patterns
    if (lower.starts_with("i am waiting for ") || lower.starts_with("waiting for "))
        && (lower.contains("task") || lower.contains("finish") || lower.contains("complete"))
    {
        return true;
    }

    // 2. Indonesian task waiting patterns
    if lower.starts_with("sedang menunggu ")
        && (lower.contains("task") || lower.contains("selesai") || lower.contains("proses kompilasi"))
    {
        return true;
    }

    // 3. Task transition patterns
    if lower.starts_with("the task has almost completed")
        || lower.starts_with("let me inspect the final output")
        || lower.starts_with("tool is running as a background task")
        || lower.starts_with("task logs are available at:")
        || lower.starts_with("you must take one of the following two actions:")
    {
        return true;
    }

    // 4. Background task completion logs & system artifacts
    if lower.starts_with("task id \"") && lower.contains("\" finished with result") {
        return true;
    }
    if lower.starts_with("the command exited with code")
        || lower.starts_with("terminal id:")
        || lower.starts_with("log: file:///")
        || lower.starts_with("[message] timestamp=")
        || lower.starts_with("the following is a <system_message>")
        || lower.starts_with("<system_message")
        || lower.starts_with("</system_message")
    {
        return true;
    }

    // 5. Tool execution confirmation and diagnostic message leakages
    if (lower.starts_with("status: terkirim") || lower.starts_with("status: berhasil"))
        && (lower.contains("message_id") || lower.contains("3eb0"))
    {
        return true;
    }
    if lower.starts_with("pengingat sudah berhasil dikirimkan ke whatsapp")
        || lower.starts_with("pesan sudah berhasil dikirimkan ke whatsapp")
        || lower.starts_with("pesan telah berhasil dikirimkan ke whatsapp")
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_agent_response_strips_intermediate_tasks() {
        let raw = r#"The task has almost completed, let me inspect the final output.
I am waiting for the metadata task to finish.
I am waiting for task-907 to complete.
I am waiting for task-914 to complete.
Bisa bangeett, Bang Ihzaa! Ini solusi yang pas banget supaya data hasil konfirmasi petugas di lapangan lewat AppSheet bisa langsung mengalir otomatis ke 9 spreadsheet sumber utama KCDA."#;

        let cleaned = sanitize_agent_response(raw);
        assert!(!cleaned.contains("task-907"));
        assert!(!cleaned.contains("The task has almost completed"));
        assert!(!cleaned.contains("metadata task to finish"));
        assert!(cleaned.starts_with("Bisa bangeett, Bang Ihzaa!"));
    }

    #[test]
    fn test_sanitize_agent_response_preserves_legitimate_waiting_text() {
        let raw = "Kami sedang menunggu konfirmasi resmi dari BPS terkait jadwal rilis KCDA.";
        let cleaned = sanitize_agent_response(raw);
        assert_eq!(cleaned, raw);
    }

    #[test]
    fn test_sanitize_agent_response_strips_system_message_blocks() {
        let raw = r#"The following is a <SYSTEM_MESSAGE> not actually sent by the user. It is provided by the system as important information to pay attention to.

<SYSTEM_MESSAGE>
[Message] timestamp=2026-09-16T13:17:10Z sender=f222b011-5303-48b6-90c7-edf593c884b5/task-1974 priority=MESSAGE_PRIORITY_HIGH content=Task id "f222b011-5303-48b6-90c7-edf593c884b5/task-1974" finished with result:

The command exited with code 0.
Output:
-rw-r--r-- 1 root root 224855 Sep 16 20:17 /tmp/monitoring_pml_wb2.png

Terminal ID: term_chrome

Log: file:///root/.gemini/antigravity-cli/brain/f222b011-5303-48b6-90c7-edf593c884b5/.system_generated/tasks/task-1974.log
</SYSTEM_MESSAGE>}
Ini yaa Bang Ihza @Ihza Karunia! Gambarnya barusan sudah langsung Aina kirimkan ke atas.

Tangkapan layar tersebut diambil langsung menggunakan browser headless bawaan pada tab *Perpml*."#;

        let cleaned = sanitize_agent_response(raw);
        assert!(!cleaned.contains("<SYSTEM_MESSAGE>"));
        assert!(!cleaned.contains("The following is a <SYSTEM_MESSAGE>"));
        assert!(!cleaned.contains("task-1974"));
        assert!(!cleaned.contains("term_chrome"));
        assert!(cleaned.starts_with("Ini yaa Bang Ihza @Ihza Karunia!"));
        assert!(cleaned.contains("Tangkapan layar tersebut diambil langsung"));
    }
}
