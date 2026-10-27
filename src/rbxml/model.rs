use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename = "DJ_PLAYLISTS")]
pub struct Document {
    #[serde(rename = "@Version")]
    pub version: String,
    #[serde(rename = "PRODUCT")]
    pub product: Product,
    #[serde(rename = "COLLECTION")]
    pub collection: Collection,
    #[serde(rename = "PLAYLISTS")]
    pub playlists: Playlists,
}
macro_rules! attributes {
    ($name:ident {$($field:ident:$ty:ty=>$attr:literal),* $(,)?})=>{
        #[derive(Clone,Debug,Default,PartialEq,Serialize,Deserialize)]
        #[serde(default)] pub struct $name {$(#[serde(rename=$attr)] pub $field:$ty,)*}
    }
}
attributes!(Product {name:String=>"@Name",version:String=>"@Version",company:String=>"@Company"});
attributes!(Tempo {start:f64=>"@Inizio",bpm:f64=>"@Bpm",meter:String=>"@Metro",beat:u8=>"@Battito"});
attributes!(PositionMark {name:String=>"@Name",kind:i32=>"@Type",start:f64=>"@Start",end:f64=>"@End",num:i32=>"@Num"});
attributes!(NodeTrack {key:String=>"@Key"});

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Track {
    #[serde(rename = "@TrackID")]
    pub id: i64,
    #[serde(rename = "@Name")]
    pub name: String,
    #[serde(rename = "@Artist")]
    pub artist: String,
    #[serde(rename = "@Composer")]
    pub composer: String,
    #[serde(rename = "@Album")]
    pub album: String,
    #[serde(rename = "@Grouping")]
    pub grouping: String,
    #[serde(rename = "@Genre")]
    pub genre: String,
    #[serde(rename = "@Kind")]
    pub kind: String,
    #[serde(rename = "@Size")]
    pub size: u64,
    #[serde(rename = "@TotalTime")]
    pub total_time: f64,
    #[serde(rename = "@DiscNumber")]
    pub disc_number: u32,
    #[serde(rename = "@TrackNumber")]
    pub track_number: u32,
    #[serde(rename = "@Year")]
    pub year: u32,
    #[serde(rename = "@AverageBpm")]
    pub average_bpm: f64,
    #[serde(rename = "@DateModified")]
    pub date_modified: String,
    #[serde(rename = "@DateAdded")]
    pub date_added: String,
    #[serde(rename = "@BitRate")]
    pub bitrate: u32,
    #[serde(rename = "@SampleRate")]
    pub sample_rate: f64,
    #[serde(rename = "@Comments")]
    pub comments: String,
    #[serde(rename = "@PlayCount")]
    pub play_count: u64,
    #[serde(rename = "@LastPlayed")]
    pub last_played: String,
    #[serde(rename = "@Rating")]
    pub rating: u16,
    #[serde(rename = "@Location")]
    pub location: String,
    #[serde(rename = "@Remixer")]
    pub remixer: String,
    #[serde(rename = "@Tonality")]
    pub tonality: String,
    #[serde(rename = "@Label")]
    pub label: String,
    #[serde(rename = "@Mix")]
    pub mix: String,
    #[serde(rename = "@Colour")]
    pub color: String,
    #[serde(rename = "TEMPO")]
    pub tempos: Vec<Tempo>,
    #[serde(rename = "POSITION_MARK")]
    pub position_marks: Vec<PositionMark>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Node {
    #[serde(rename = "@Type")]
    pub node_type: u8,
    #[serde(rename = "@Name")]
    pub name: String,
    #[serde(rename = "@Count")]
    pub count: usize,
    #[serde(rename = "@Entries")]
    pub entries: usize,
    #[serde(rename = "@KeyType")]
    pub key_type: u8,
    #[serde(rename = "TRACK")]
    pub tracks: Vec<NodeTrack>,
    #[serde(rename = "NODE")]
    pub children: Vec<Node>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Collection {
    #[serde(rename = "@Entries")]
    pub entries: usize,
    #[serde(rename = "TRACK")]
    pub tracks: Vec<Track>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Playlists {
    #[serde(rename = "NODE")]
    pub node: Node,
}
