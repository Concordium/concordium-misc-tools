// firebase-messaging-sw.js
// Give the service worker access to Firebase Messaging.
// Replace 10.12.2 with the version of the Firebase JS SDK you're using in your app.
importScripts('https://www.gstatic.com/firebasejs/10.12.2/firebase-app-compat.js');
importScripts('https://www.gstatic.com/firebasejs/10.12.2/firebase-messaging-compat.js');

// Initialize the Firebase app in the service worker by passing in
// your app's Firebase config object.
// IMPORTANT: Replace with YOUR actual project config from Firebase console
// Go to Project settings > Your apps > Select your web app > Config
const firebaseConfig = {
    apiKey: "YOUR_API_KEY",
    authDomain: "YOUR_PROJECT_ID.firebaseapp.com",
    projectId: "YOUR_PROJECT_ID",
    storageBucket: "YOUR_PROJECT_ID.appspot.com",
    messagingSenderId: "YOUR_MESSAGING_SENDER_ID", // CRUCIAL for FCM
    appId: "YOUR_APP_ID"
};
firebase.initializeApp(firebaseConfig);

// Retrieve an instance of Firebase Messaging so that it can handle
// background messages.
const messaging = firebase.messaging();

// Add logic to handle background messages (e.g., display a notification)
messaging.onBackgroundMessage((payload) => {
  console.log('[firebase-messaging-sw.js] Received background message ', payload);
  // Customize notification here
  const notificationTitle = payload.notification.title;
  const notificationOptions = {
    body: payload.notification.body,
    icon: '/firebase-logo.png' // Replace with your own icon URL
  };
  self.registration.showNotification(notificationTitle, notificationOptions);
});
