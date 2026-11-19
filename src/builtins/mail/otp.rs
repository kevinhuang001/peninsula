const OTP_TERMS: &[&str] = &[
    // Chinese (simplified and traditional)
    "验证码",
    "驗證碼",
    "校验码",
    "校驗碼",
    "安全码",
    "安全碼",
    "动态密码",
    "動態密碼",
    "一次性密码",
    "一次性密碼",
    "登录码",
    "登錄碼",
    // Japanese
    "認証コード",
    "確認コード",
    "ワンタイムパスワード",
    "ワンタイムコード",
    "セキュリティコード",
    "認証番号",
    // Korean
    "인증 코드",
    "인증코드",
    "인증번호",
    "보안 코드",
    "보안코드",
    "일회용 비밀번호",
    "확인 코드",
    "확인코드",
    // English
    "verification code",
    "verify code",
    "verification token",
    "one-time password",
    "one time password",
    "one-time code",
    "one time code",
    "security code",
    "authentication code",
    "auth code",
    "login code",
    "confirmation code",
    "confirm code",
    "passcode",
    "pass code",
    "access code",
    "otp",
    "2fa",
    "mfa",
    // French
    "code de vérification",
    "code de verification",
    "code de sécurité",
    "code de securite",
    "code d'authentification",
    "mot de passe à usage unique",
    "mot de passe a usage unique",
    "code de confirmation",
    // Spanish
    "código de verificación",
    "codigo de verificacion",
    "código de seguridad",
    "codigo de seguridad",
    "contraseña de un solo uso",
    "contrasena de un solo uso",
    "código de acceso",
    "codigo de acceso",
    // Portuguese
    "código de verificação",
    "codigo de verificacao",
    "código de segurança",
    "codigo de seguranca",
    "senha de uso único",
    "senha de uso unico",
    // German
    "bestätigungscode",
    "bestatigungscode",
    "verifizierungscode",
    "sicherheitscode",
    "einmalpasswort",
    "einmaliger code",
    "code zur bestätigung",
    "code zur bestatigung",
    // Italian
    "codice di verifica",
    "codice di sicurezza",
    "password monouso",
    "codice di conferma",
    // Dutch
    "verificatiecode",
    "beveiligingscode",
    "eenmalig wachtwoord",
    "bevestigingscode",
    // Finnish, Greek, Czech, Romanian, and Hungarian
    "vahvistuskoodi",
    "turvakoodi",
    "kertakäyttöinen salasana",
    "κωδικός επαλήθευσης",
    "κωδικός ασφαλείας",
    "κωδικός επιβεβαίωσης",
    "ověřovací kód",
    "bezpečnostní kód",
    "jednorázové heslo",
    "potvrzovací kód",
    "cod de verificare",
    "cod de securitate",
    "parolă de unică folosință",
    "cod de confirmare",
    "megerősítő kód",
    "ellenőrző kód",
    "egyszer használatos jelszó",
    // Russian
    "код подтверждения",
    "код проверки",
    "код безопасности",
    "одноразовый пароль",
    "код входа",
    "код підтвердження",
    "код перевірки",
    // Arabic
    "رمز التحقق",
    "رمز التأكيد",
    "رمز الأمان",
    "كلمة المرور لمرة واحدة",
    // Hebrew
    "קוד אימות",
    "קוד אבטחה",
    "סיסמה חד-פעמית",
    // Turkish
    "doğrulama kodu",
    "guvenlik kodu",
    "tek kullanımlık şifre",
    "onay kodu",
    // Polish
    "kod weryfikacyjny",
    "kod potwierdzający",
    "hasło jednorazowe",
    // Swedish, Danish, Norwegian
    "verifieringskod",
    "säkerhetskod",
    "bekräftelsekod",
    "engångskod",
    "bekræftelseskode",
    "bekreftelseskode",
    "engangskode",
    "verifiseringskode",
    // Hindi, Thai, Vietnamese, Indonesian, and Malay
    "सत्यापन कोड",
    "पुष्टि कोड",
    "सुरक्षा कोड",
    "एक बार का पासवर्ड",
    "รหัสยืนยัน",
    "รหัสตรวจสอบ",
    "รหัสความปลอดภัย",
    "mã xác minh",
    "mã xác thực",
    "mã xác nhận",
    "mật khẩu dùng một lần",
    "kode verifikasi",
    "kode keamanan",
    "kata sandi sekali pakai",
    "kod pengesahan",
    "kod keselamatan",
];

pub(super) fn detect(subject: &str, body: &str) -> Option<String> {
    let text = format!("{} {}", subject.trim(), body.trim())
        .replace('\n', " ")
        .replace('\r', " ");
    for sentence in split_sentences(&text) {
        let (folded, original_positions) = lowercase_with_positions(sentence);
        let mut candidates = None;
        for term in OTP_TERMS {
            for (start, _) in folded.match_indices(*term) {
                if ascii_term(*term) && !has_ascii_word_boundaries(&folded, start, term.len()) {
                    continue;
                }
                let start_char = folded[..start].chars().count();
                let end_char = folded[..start + term.len()].chars().count();
                let original_start = original_positions
                    .get(start_char)
                    .copied()
                    .unwrap_or(sentence.len());
                let original_end = original_positions
                    .get(end_char)
                    .copied()
                    .unwrap_or(sentence.len());
                let candidates = candidates.get_or_insert_with(|| code_tokens(sentence));
                if let Some(code) = find_nearby_code(
                    original_start,
                    original_end.saturating_sub(original_start),
                    candidates,
                ) {
                    return Some(code);
                }
            }
        }
    }
    None
}

fn lowercase_with_positions(text: &str) -> (String, Vec<usize>) {
    let mut folded = String::with_capacity(text.len());
    let mut original_positions = Vec::new();
    for (index, character) in text.char_indices() {
        let lower: String = character.to_lowercase().collect();
        original_positions.extend(std::iter::repeat(index).take(lower.chars().count()));
        folded.push_str(&lower);
    }
    (folded, original_positions)
}

fn split_sentences(text: &str) -> Vec<&str> {
    let mut sentences = Vec::new();
    let mut start = 0;
    for (index, character) in text.char_indices() {
        if matches!(character, '\n' | '\r' | '.' | '!' | '?' | '。' | '！' | '？') {
            let end = index + character.len_utf8();
            if !text[start..end].trim().is_empty() {
                sentences.push(&text[start..end]);
            }
            start = end;
        }
    }
    if !text[start..].trim().is_empty() {
        sentences.push(&text[start..]);
    }
    sentences
}

fn ascii_term(term: &str) -> bool {
    term.is_ascii()
}

fn has_ascii_word_boundaries(text: &str, start: usize, length: usize) -> bool {
    let end = start + length;
    let before_is_word = text[..start]
        .chars()
        .next_back()
        .is_some_and(|character| character.is_ascii_alphanumeric());
    let after_is_word = text[end..]
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_alphanumeric());
    !before_is_word && !after_is_word
}

fn find_nearby_code(
    term_start: usize,
    term_length: usize,
    candidates: &[(usize, usize, String)],
) -> Option<String> {
    let term_end = term_start + term_length;
    candidates
        .iter()
        .filter_map(|(start, end, code)| {
            let distance = if *start >= term_end {
                *start - term_end
            } else if *end <= term_start {
                term_start - *end
            } else {
                0
            };
            (distance <= 180).then_some((*start < term_start, distance, code.as_str().to_owned()))
        })
        .min_by_key(|(before_term, distance, code)| {
            (*before_term, *distance, std::cmp::Reverse(code.chars().count()))
        })
        .map(|(_, _, code)| code)
}

fn code_tokens(sentence: &str) -> Vec<(usize, usize, String)> {
    let mut tokens = Vec::new();
    let mut numeric_fragments = Vec::new();
    let mut start = None;
    let mut value = String::new();
    let mut has_letter = false;
    let mut digit_count = 0;
    for (index, character) in sentence.char_indices() {
        if let Some(normalized) = normalize_code_character(character) {
            start.get_or_insert(index);
            value.push(normalized);
            if normalized.is_ascii_digit() {
                digit_count += 1;
            } else {
                has_letter = true;
            }
        } else if let Some(token_start) = start.take() {
            if valid_code(&value, has_letter, digit_count) {
                tokens.push((token_start, index, value.clone()));
            }
            if !has_letter && (1..=8).contains(&digit_count) {
                numeric_fragments.push((token_start, index, value.clone()));
            }
            value.clear();
            has_letter = false;
            digit_count = 0;
        }
    }
    if let Some(token_start) = start {
        if valid_code(&value, has_letter, digit_count) {
            tokens.push((token_start, sentence.len(), value.clone()));
        }
        if !has_letter && (1..=8).contains(&digit_count) {
            numeric_fragments.push((token_start, sentence.len(), value));
        }
    }
    let mut grouped = Vec::new();
    for start_index in 0..numeric_fragments.len() {
        let (start, original_end, first) = &numeric_fragments[start_index];
        if !first.chars().all(|character| character.is_ascii_digit()) {
            continue;
        }
        if valid_code(first, false, first.chars().count()) {
            continue;
        }
        let mut value = first.as_str().to_owned();
        let mut end = *original_end;
        for (next_start, next_end, next_value) in numeric_fragments.iter().skip(start_index + 1) {
            let separator = &sentence[end..*next_start];
            if separator.is_empty()
                || !separator.chars().all(char::is_whitespace)
                || !next_value.chars().all(|character| character.is_ascii_digit())
                || value.len() + next_value.len() > 8
            {
                break;
            }
            value.push_str(next_value);
            end = *next_end;
            if (4..=8).contains(&value.len()) {
                grouped.push((*start, end, value.clone()));
            }
        }
    }
    tokens.extend(grouped);
    tokens
}

fn normalize_code_character(character: char) -> Option<char> {
    if character.is_ascii_alphanumeric() {
        Some(character)
    } else if let Some(value) = unicode_code_point(character) {
        char::from_u32(value)
    } else if let Some(digit) = character.to_digit(10) {
        char::from_digit(digit, 10)
    } else {
        None
    }
}

fn unicode_code_point(character: char) -> Option<u32> {
    let point = character as u32;
    if (0xFF10..=0xFF19).contains(&point) {
        return Some(point - 0xFF10 + u32::from(b'0'));
    }
    if (0xFF21..=0xFF3A).contains(&point) {
        return Some(point - 0xFF21 + u32::from(b'A'));
    }
    if (0xFF41..=0xFF5A).contains(&point) {
        return Some(point - 0xFF41 + u32::from(b'a'));
    }
    [
        0x0660, 0x06F0, 0x0966, 0x09E6, 0x0A66, 0x0AE6, 0x0B66, 0x0BE6, 0x0C66, 0x0CE6,
        0x0D66, 0x0E50, 0x0ED0,
    ]
    .into_iter()
    .find_map(|start| (start..start + 10).contains(&point).then_some(point - start + u32::from(b'0')))
}

fn valid_code(value: &str, has_letter: bool, digit_count: usize) -> bool {
    if has_letter {
        (4..=10).contains(&value.len()) && (2..=8).contains(&digit_count)
    } else {
        (4..=8).contains(&digit_count)
    }
}
