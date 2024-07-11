// use webhook::client::WebhookClient;

use std::error::Error;

use anyhow::anyhow;
use crate::database::models::{player::SimplePlayer, punishment::{Punishment, StaffNote}};
use serde::Serialize;

pub struct WebhookUtils {
    pub reports_webhook_client: Option<WebhookClient>,
    pub punishments_webhook_client: Option<WebhookClient>,
    pub notes_webhook_client: Option<WebhookClient>
}

impl WebhookUtils {
    const COLOR_NEW_REPORT : u32 = 0xFFEE00;
    const COLOR_NEW_PUNISHMENT : u32 = 0x0077FF;
    const COLOR_PUNISHMENT_REVERTED : u32 = 0x00FF4C;
    const COLOR_NEW_NOTE : u32 = 0xFF77FF;
    const COLOR_DEL_NOTE : u32 = 0xFF4F55;

    pub fn new(
        reports_webhook_url: &Option<String>, 
        punishments_webhook_url: &Option<String>,
        notes_webhook_url: &Option<String>
    ) -> Self {
        Self {
            reports_webhook_client: reports_webhook_url.as_ref().map(|url| {
                WebhookClient { url: url.to_owned(), client: reqwest::Client::new() }
            }),
            punishments_webhook_client: punishments_webhook_url.as_ref().map(|url| {
                WebhookClient { url: url.to_owned(), client: reqwest::Client::new() }
            }),
            notes_webhook_client: notes_webhook_url.as_ref().map(|url| {
                WebhookClient { url: url.to_owned(), client: reqwest::Client::new() }
            })
        }
    }

    pub async fn send_report_webhook(
        &self, 
        server_id: &String, 
        reporter: &SimplePlayer, 
        target: &SimplePlayer, 
        reason: &String, 
        online_staff: &Vec<SimplePlayer>
    ) {
        if let Some(reports_client) = &self.reports_webhook_client {
            let mut embed = DiscordEmbed::default();
            embed
                .color(Self::COLOR_NEW_REPORT)
                .title(format!("New report (on {})", server_id))
                .thumbnail(target.get_mini_icon_url())
                .footer(DiscordEmbedFooter { 
                    text: format!("Reported by {}", &reporter.name), 
                    icon_url: Some(reporter.get_mini_icon_url())
                })
                .add_field(
                    DiscordEmbedField { 
                        name: String::from("Player"), 
                        value: target.name.clone(), 
                        inline: true 
                    }
                )
                .add_field(
                    DiscordEmbedField { 
                        name: String::from("Reason"), 
                        value: escape_markdown(reason, false), 
                        inline: true 
                    }
                )
                .add_field(
                    DiscordEmbedField { 
                        name: String::from("Online staff"), 
                        value: online_staff
                            .iter()
                            .map(|s| s.name.to_owned())
                            .collect::<Vec<String>>()
                            .join("\n"), 
                        inline: false 
                    }
                );
            reports_client.send(
                &WebhookMessage::default().add_embed(embed)
            ).await;
        }
    }

    pub async fn send_punishment_webhook(
        &self, 
        punishment: &Punishment
    ) {
        if let Some(punishments_client) = &self.punishments_webhook_client {
            let mut embed = DiscordEmbed::default();
            embed
                .color(Self::COLOR_NEW_PUNISHMENT)
                .title(String::from("New punishment"))
                .footer(DiscordEmbedFooter { 
                    text: format!("Pun ID: {}", punishment.id), 
                    icon_url: None 
                })
                .thumbnail(punishment.target.get_mini_icon_url())
                .add_field(
                    DiscordEmbedField { 
                        name: String::from("Target"), 
                        value: punishment.target.name.to_owned(),
                        inline: true 
                    }
                )
                .add_field(
                    DiscordEmbedField { 
                        name: String::from("Staff"), 
                        value: 
                            if let Some(punisher) = &punishment.punisher { punisher.name.to_owned() } 
                            else { String::from("Console") },
                        inline: true 
                    }
                )
                .add_field(
                    DiscordEmbedField { 
                        name: String::from("Type"), 
                        value: punishment.action.kind.to_string(),
                        inline: true 
                    }
                )
                .add_field(
                    DiscordEmbedField { 
                        name: String::from("Reason"), 
                        value: format!("{} ({})", 
                            escape_markdown(&punishment.reason.name, false), punishment.offence
                        ),
                        inline: false 
                    }
                );
            if let Some(note) = &punishment.note {
                embed.add_field(DiscordEmbedField { 
                    name: String::from("Note"), 
                    value: note.to_owned(), 
                    inline: true
                });
            }
            punishments_client.send(
                &WebhookMessage::default().add_embed(embed)
            ).await;
        }
    }

    pub async fn send_punishment_reversion_webhook(
        &self, 
        punishment: &Punishment
    ) {
        match &punishment.reversion {
            None => return,
            Some(reversion) => {
                if let Some(punishments_client) = &self.punishments_webhook_client {
                    let mut embed = DiscordEmbed::default();
                    embed
                        .color(Self::COLOR_PUNISHMENT_REVERTED)
                        .title(String::from("Punishment reverted"))
                        .footer(DiscordEmbedFooter { 
                            text: format!("Pun ID: {}", punishment.id), 
                            icon_url: None 
                        })
                        .thumbnail(punishment.target.get_mini_icon_url())
                        .add_field(
                            DiscordEmbedField { 
                                name: String::from("Target"), 
                                value: punishment.target.name.to_owned(),
                                inline: true 
                            }
                        )
                        .add_field(
                            DiscordEmbedField { 
                                name: String::from("Staff"), 
                                value: 
                                    if let Some(punisher) = &punishment.punisher { punisher.name.to_owned() } 
                                    else { String::from("Console") },
                                inline: true 
                            }
                        )
                        .add_field(
                            DiscordEmbedField { 
                                name: String::from("Punishment"), 
                                value: format!("{} - {} ({})", 
                                   punishment.action.kind.to_string(), 
                                   escape_markdown(&punishment.reason.name, false), 
                                   punishment.offence
                                ),
                                inline: false 
                            }
                        )
                        .add_field(
                            DiscordEmbedField { 
                                name: String::from("Reversion reason"), 
                                value: escape_markdown(&reversion.reason, false),
                                inline: false 
                            }
                        );
                    punishments_client.send(
                        &WebhookMessage::default().add_embed(embed)
                    ).await;
                }
            }
        };
    }

    pub async fn send_new_note_webhook(
        &self, 
        player: &SimplePlayer,
        note: &StaffNote
    ) {
        if let Some(notes_client) = &self.notes_webhook_client {
            let mut embed = DiscordEmbed::default();
            embed
                .color(Self::COLOR_NEW_NOTE)
                .title(String::from("Note added"))
                .footer(DiscordEmbedFooter { 
                    text: format!("Player note ID: {}", note.id), 
                    icon_url: None 
                })
                .thumbnail(player.get_mini_icon_url())
                .add_field(
                    DiscordEmbedField { 
                        name: String::from("Target"), 
                        value: escape_markdown(&player.name, false),
                        inline: true 
                    }
                )
                .add_field(
                    DiscordEmbedField { 
                        name: String::from("Staff"), 
                        value: escape_markdown(&note.author.name, false),
                        inline: true 
                    }
                )
                .add_field(
                    DiscordEmbedField { 
                        name: String::from("Note"), 
                        value: escape_markdown(&note.content, false),
                        inline: true 
                    }
                );
            notes_client.send(
                &WebhookMessage::default().add_embed(embed)
            ).await;
        }
    }

    pub async fn send_deleted_note_webhook(
        &self, 
        player: &SimplePlayer,
        note: &StaffNote
    ) {
        if let Some(notes_client) = &self.notes_webhook_client {
            let mut embed = DiscordEmbed::default();
            embed
                .color(Self::COLOR_DEL_NOTE)
                .title(String::from("Note deleted"))
                .footer(DiscordEmbedFooter { 
                    text: format!("Player note ID: {}", note.id), 
                    icon_url: None 
                })
                .thumbnail(player.get_mini_icon_url())
                .add_field(
                    DiscordEmbedField { 
                        name: String::from("Target"), 
                        value: escape_markdown(&player.name, false),
                        inline: true 
                    }
                )
                .add_field(
                    DiscordEmbedField { 
                        name: String::from("Staff"), 
                        value: escape_markdown(&note.author.name, false),
                        inline: true 
                    }
                )
                .add_field(
                    DiscordEmbedField { 
                        name: String::from("Note"), 
                        value: escape_markdown(&note.content, false),
                        inline: true 
                    }
                );
            notes_client.send(
                &WebhookMessage::default().add_embed(embed)
            ).await;
        }
    }

}

fn escape_markdown(s: &String, html_mode: bool) -> String {
    let mut escaped = s
        .replace("*", "\\*")
        .replace("/", "\\/")
        .replace("(", "\\(")
        .replace(")", "\\)")
        .replace("[", "\\[")
        .replace("]", "\\]")
        .replace("_", "\\_");
    if html_mode {
        escaped = s
            .replace("#", "\\#")
            .replace("<", "&lt;")
            .replace(">", "&gt;");
    }
    escaped
}

pub struct WebhookClient {
    pub url: String,
    pub client: reqwest::Client
}

impl WebhookClient {
    pub async fn send(&self, message: &WebhookMessage) -> anyhow::Result<()> {
        if message.content.is_none() && message.embeds.is_empty() {
            return Err(anyhow!(
                "At least one of 'content' or 'embeds' must be present"
            ));
        }
        match self.client.post(&self.url)
            .json(message)
            .send()
            .await {
                Err(e) => {
                    warn!("Webhook failed: {}", e);
                },
                Ok(_) => {}
        };
        Ok(())
    }
}

#[derive(Serialize)]
pub struct WebhookMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
    tts: bool,
    embeds: Vec<DiscordEmbed>
}

impl Default for WebhookMessage {
    fn default() -> Self {
        Self { 
            content: None, 
            username: None, 
            avatar_url: None, 
            tts: false, 
            embeds: Vec::new()
        }
    }
}

impl WebhookMessage {
    pub fn content(&mut self, content: String) -> &mut Self {
        self.content = Some(content);
        self
    }

    pub fn username(&mut self, username: String) -> &mut Self {
        self.username = Some(username);
        self
    }

    pub fn add_embed(&mut self, embed: DiscordEmbed) -> &mut Self {
        self.embeds.push(embed);
        self
    }
}

#[derive(Serialize)]
pub struct DiscordEmbed {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(rename = "type")]
    embed_type: String,
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    footer: Option<DiscordEmbedFooter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thumbnail: Option<DiscordEmbedThumbnail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fields: Option<Vec<DiscordEmbedField>>
}

impl Default for DiscordEmbed {
    fn default() -> Self {
        Self {
            title: None,
            embed_type: String::from("rich"),
            description: None,
            url: None,
            timestamp: None,
            color: None,
            footer: None,
            thumbnail: None,
            fields: None
        }
    }
}

impl DiscordEmbed {
    pub fn title(&mut self, title: String) -> &mut Self {
        self.title = Some(title);
        self
    }

    pub fn description(&mut self, description: String) -> &mut Self {
        self.description = Some(description);
        self
    }

    pub fn url(&mut self, url: String) -> &mut Self {
        self.url = Some(url);
        self
    }

    pub fn color(&mut self, color: u32) -> &mut Self {
        self.color = Some(color);
        self
    }

    pub fn footer(&mut self, footer: DiscordEmbedFooter) -> &mut Self {
        self.footer = Some(footer);
        self
    }

    pub fn thumbnail(&mut self, thumbnail: String) -> &mut Self {
        self.thumbnail = Some(DiscordEmbedThumbnail { url: thumbnail });
        self
    }

    pub fn add_field(&mut self, field: DiscordEmbedField) -> &mut Self {
        if self.fields.is_none() {
            self.fields = Some(Vec::new());
        }

        self.fields.as_mut().unwrap().push(field);
        self
    }
}

#[derive(Serialize)]
pub struct DiscordEmbedFooter {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>
}

#[derive(Serialize)]
pub struct DiscordEmbedThumbnail {
    pub url: String
}

#[derive(Serialize)]
pub struct DiscordEmbedField {
    pub name: String,
    pub value: String,
    pub inline: bool
}
