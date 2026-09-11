use super::message::{ChatType, IncomingMessage};

pub struct PersonaEngine {
    persona_text: String,
}

impl PersonaEngine {
    pub fn new(persona_text: String) -> Self {
        Self { persona_text }
    }

    /// Prepares the complete prompt injected into the Antigravity agent CLI.
    pub fn build_prompt(&self, msg: &IncomingMessage) -> String {
        let sender_name = msg
            .sender
            .name
            .as_deref()
            .unwrap_or("Rekan Kerja");
        let sender_jid = &msg.sender.jid;
        
        let chat_context_str = match msg.chat_type {
            ChatType::DirectMessage => "Pesan Pribadi (DM/Japri)".to_string(),
            ChatType::Group => format!("Grup WhatsApp ({})", msg.chat_jid),
        };

        let quoted_context = match &msg.quoted_message {
            Some(q) => format!(
                "\n[Pesan yang di-quote/reply]:\nDari {}: \"{}\"\n",
                q.sender_jid, q.text
            ),
            None => String::new(),
        };

        format!(
            "{persona}\n\n\
            ---\n\
            [Konteks Percakapan Masuk]\n\
            - Ruang Obrolan: {chat_context}\n\
            - Pengirim: {sender_name} ({sender_jid})\n\
            {quoted_context}\
            \n\
            [Pesan dari Pengirim]:\n\
            {text}\n\n\
            [Instruksi Respons]:\n\
            - Balaslah secara langsung sebagai Aina kepada {sender_name}.\n\
            - Ingat: ramah, cekatan, solutif, basa-basi seperlunya.\n\
            - Jika permintaan pengirim kurang jelas, kurang spesifikasi/parameter, atau ambigu, tanyakan klarifikasi secara sopan dan terarah.\n\
            - Gunakan formatting WhatsApp (*tebal*, _miring_, `kode`).",
            persona = self.persona_text,
            chat_context = chat_context_str,
            sender_name = sender_name,
            sender_jid = sender_jid,
            quoted_context = quoted_context,
            text = msg.text
        )
    }
}
