//! This module provides tips for using social media effectively.

use leptos::prelude::*;
#[cfg(feature = "ssr")]
use rand::Rng;

/// A component that displays tips for using social media effectively.
#[component]
pub fn SocialMediaTips() -> impl IntoView {
    let tips = vec![
        "Check out our new release... <INSERT RELEASE HERE> Out on Friday!",
        "Post regularly to keep your audience engaged.",
        "Use high-quality images and videos to attract attention.",
        "Engage with your followers by responding to comments and messages.",
        "Utilize hashtags to increase the visibility of your posts.",
        "Analyze your social media metrics to understand what works best.",
        "Collaborate with influencers to reach a wider audience.",
        "Post on threads to engage with your community more effectively.",
        "Create shareable content to encourage reposts and mentions.",
        "Use trending hashtags to boost post engagement during popular times.",
        "Engage in relevant discussions and participate in group chats to build relationships.",
        "Schedule posts in advance to manage your posting timeline efficiently.",
        "Monitor the performance of your posts with tools that track reach, engagement, and conversions.",
        "Use visually appealing graphics to enhance the aesthetics of your social media content.",
        "Engage with other influencers in your niche for potential collaborations or cross-promotions.",
        "Utilize stories to add variety to your regular posts and keep followers interested.",
        "Create contests or giveaways to increase user engagement and attract new followers.",
        "Share user-generated content to show appreciation and encourage more contributions.",
        "Monitor trends in your industry and adapt your social media strategy accordingly.",
        "Use polls and quizzes as a fun way to interact with your audience and gather feedback.",
        "Get hyped for <ARTIST>'s official visualizer of '<RELEASE>' – it's coming Friday!",
        "What track are you most excited about? Let us know now by commenting on this post about <RELEASE>.",
        "Get hyped for <ARTIST>'s official visualizer of '<RELEASE>' – it's coming Friday!",
        "What track are you most excited about? Let us know now by commenting on this post about <RELEASE>.",
        "<ARTIST>'s new album, '<RELEASE>', featuring singles like '<TRACK_NAME>' and release date is <RELEASE_DATE>, drops soon! Who else can't wait?",
        "Countdown Alert: Only 2 days to listen to the debut of our <RELEASE> series. You'll be hearing from <ARTIST> every other day.",
        "Drop a ❤️ for each track you'll explore in the upcoming <RELEASE> visualizer countdown – it's art we're sharing soon!",
        "<TRACK_NAME>, the lead single off '<RELEASE>', is already shaping up to be viral! Be sure not to miss its debut and engagement prompt on release day.",
        "Our exclusive interview with <ARTIST> about '<RELEASE>' coming soon? Comment now telling us your music question for a hint or reveal below!",
        "Exclusive Tease: A leaked clip of the main visualizer countdown from <RELEASE> is out. Check it via comments to share this moment, and tag friends who'll enjoy more engagement.",
        "<RELEASE_DATE> marks our biggest launch yet! We're announcing '<TRACK_NAME>' now on social – we'd love to hear what you think!",
        "Artist Spotlight Alert: Behind the scenes from <ARTIST>, we've revealed 'TrackName' is one of many hits in the upcoming release. Vote down below which song you'll share most!",
        "The world's getting ready for '<RELEASE>' on November 15th (example date). Are you listening? Our tips will help boost your engagement levels.",
        "<ARTIST> drops a new lyric video teaser today from us – repost or save it to stay in the loop and promote even more.",
        "Coming Soon: Countdown! We've seen 'TrackName' previewed by fans, and I know your <RELEASE> date is just days away. Let's share that with others for high engagement!",
        "The visualizer countdown of '<RELEASE>' from us? It might have some special filter effects you'll see only on release day – comment what effect excites most.",
        "New Music Update: An early audio sample of one song in <RELEASE> leaked. Comment away telling us how engaged this keeps your audience! Don't repost if not sure, wait for the official announcement.",
        "<ARTIST>'s main music video teaser released today via our platform – what's your guess about the theme or songs included? Vote now!",
        "We're Officially Spilling Beans On: The release date is <RELEASE_DATE> for '<RELEASE>', and one of my favorite tracks is called '<TRACK_NAME>'. Who knew we'd be driving engagement like this all month?",
        "This upcoming <RELEASE>, including singles, dates, etc., needs your community participation. How? Share with everyone using our interactive posts – reply below!",
    ];

    #[cfg(not(feature = "ssr"))]
    let index = js_sys::Math::floor(js_sys::Math::random() * tips.len() as f64) as usize;

    #[cfg(feature = "ssr")]
    let index = {
        let mut rng = rand::rng();
        rng.random_range(0..tips.len())
    };

    let tip = tips[index];

    view! {
        <div class="overflow-x-auto shadow-xl basis-sm not-prose card bg-neutral text-neutral-content bg-base-100">
            <div class="card-body">
                <h2 class="card-title">Social Tips!</h2>
                <p>"Tips for promotion and social engagement…"</p>
                <p>{tip}</p>
                <div class="justify-end card-actions"></div>
            </div>
        </div>
    }
}
